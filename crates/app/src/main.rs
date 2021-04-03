mod checked_path;
mod db;
mod pdf_to_images;
mod utils;
mod video_exts;
mod web;

use anyhow::{Context, Result};
use checked_path::{CheckedPath, Kind};
use db::DbPool;
use dialoguer::Confirm;
use indicatif::{ProgressBar, ProgressStyle};
use matching::{ImageVideoMatcher, MatchableImage, Matching, ProgressReporter};
use matching_opencv::OpenCVImageVideoMatcher;
use pdf_to_images::{pdfs_to_images, PdfPage};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::{collections::HashSet, sync::Mutex};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use structopt::StructOpt;
use utils::hash_file;
use web::start_server;

#[derive(StructOpt, Debug)]
#[structopt(name = "slideo")]
struct Opt {
    /// A list of all videos and pdfs to process. If only a single pdf is passed, opens a viewer.
    #[structopt(name = "FILES", parse(from_os_str), required = true)]
    files: Vec<PathBuf>,

    /// Invalidates any cached mapping entries that exist for the given files.
    #[structopt(long)]
    invalidate_video_cache: bool,

    /// Does not wait for user input.
    #[structopt(long)]
    non_interactive: bool,
}

#[derive(Clone, Debug)]
pub struct HashedFile {
    pub path: PathBuf,
    pub hash: String,
}

impl HashedFile {
    pub fn new(path: PathBuf, hash: String) -> HashedFile {
        HashedFile { path, hash }
    }
}

#[async_std::main]
async fn main() -> Result<()> {
    let opt: Opt = Opt::from_args();

    let paths = opt
        .files
        .into_iter()
        .map(CheckedPath::from)
        .collect::<Result<Vec<CheckedPath>>>()?;

    let paths = paths
        .into_par_iter()
        .map(|p| {
            let hash = hash_file(&p.path)
                .with_context(|| format!("Could not hash file {}", p.path.to_string_lossy()))?;
            Ok(CheckedPath {
                hash: Some(hash),
                kind: p.kind,
                path: p.path,
            })
        })
        .collect::<Result<Vec<CheckedPath>>>()?;

    let db_pool = DbPool::connect().await?;
    let mut db = db_pool.db().await?;

    let mut tx = db.begin_trans().await?;
    tx.update_hashes(
        paths
            .iter()
            .map(|p| (p.path.as_ref(), p.hash.as_ref().unwrap() as &str)),
    )
    .await?;
    tx.commit().await?;

    let mut videos = Vec::<HashedFile>::new();
    let mut pdfs = Vec::<HashedFile>::new();

    for path in paths {
        let kind = path.kind;
        let file = HashedFile::new(path.path, path.hash.unwrap());
        if kind == Kind::Video {
            videos.push(file);
        } else if kind == Kind::Pdf {
            pdfs.push(file);
        }
    }

    let pdf_hashes: HashSet<&str> = pdfs.iter().map(|p| &p.hash as &str).collect();

    let mut videos_to_process = Vec::<HashedFile>::new();

    for video in videos {
        match db.find_mapping_info(&video.hash).await? {
            Some(existing) if !opt.invalidate_video_cache => {
                if !existing.finished {
                    if Confirm::new()
                        .with_prompt(format!(
                            "Video '{}' is currently being processed. Recompute?",
                            video.path.to_string_lossy()
                        ))
                        .interact()?
                    {
                        videos_to_process.push(video);
                    } else {
                        println!("Skipping Video.");
                    }
                } else {
                    let cached_pdf_hashes: HashSet<&str> =
                        existing.pdf_hashes.iter().map(|h| &h as &str).collect();

                    if !pdf_hashes.is_subset(&cached_pdf_hashes) {
                        if opt.non_interactive {
                            println!("Recomputing Video '{}', as it has been analyzed with different pdfs.", video.path.to_string_lossy());
                            videos_to_process.push(video);
                        }
                        else if Confirm::new()
                            .with_prompt(format!(
                                "Video '{}' has been cached, but different pdfs are provided now. Recompute?",
                                video.path.to_string_lossy()
                            ))
                            .interact()?
                        {
                            videos_to_process.push(video);
                        } else {
                            println!("Skipping Video.");
                        }
                    } else {
                        println!(
                            "Video '{}' has already been cached, skipping.",
                            video.path.to_string_lossy()
                        );
                    }
                }
            }
            _ => {
                videos_to_process.push(video);
            }
        }
    }

    //println!("Extracting pdf pages...");
    let reporter = IndicatifProgressReporter::default();

    let pages = pdfs_to_images(
        &pdfs.iter().map(|p| p).collect(),
        &db_pool,
        reporter.get_reporter(),
    )?;
    reporter.finish();

    println!("Analyzing frames...");

    let mut tx = db.begin_trans().await?;
    for video in &videos_to_process {
        tx.create_or_reset_video(&video.hash, pdfs.iter().map(|v| &v.hash as &str))
            .await?;
    }
    tx.commit().await?;

    let matcher = OpenCVImageVideoMatcher::default();

    let video_matcher = matcher.create_video_matcher(pages.iter().collect());

    let base_reporter = IndicatifProgressReporter::default();
    let reporter = ComposedProgressReporter::new(base_reporter.get_reporter());

    let tasks: Vec<_> = videos_to_process
        .iter()
        .map(|video| {
            (
                video,
                video_matcher.match_images_with_video(&video.path, reporter.create_nested()),
            )
        })
        .collect();

    for (video, task) in &tasks {
        let matchings: Vec<Matching<&PdfPage>> = task.process();

        let mut tx = db.begin_trans().await?;
        tx.update_video_matchings(&video.hash, matchings.iter())
            .await?;
        tx.commit().await?;
    }
    base_reporter.finish();

    if !opt.non_interactive && pdfs.len() == 1 {
        let first = pdfs.iter().next();
        start_server(first.map(|h| h.hash.clone()))?;
    }

    Ok(())
}

impl<'a> MatchableImage for &PdfPage<'a> {
    fn get_path(&self) -> &Path {
        &self.image_path
    }
}

struct ComposedProgressReporter {
    progress: Arc<Mutex<Vec<(u64, u64)>>>,
    inner: ProgressReporter,
}

impl ComposedProgressReporter {
    pub fn new(inner: ProgressReporter) -> Self {
        ComposedProgressReporter {
            progress: Arc::new(Mutex::new(Vec::new())),
            inner,
        }
    }
}

impl ComposedProgressReporter {
    pub fn create_nested(&self) -> ProgressReporter {
        let mut p = self.progress.lock().unwrap();
        let idx = p.len();
        p.push((0, 0));
        let p = self.progress.clone();
        let inner = self.inner.clone();

        ProgressReporter::new(Arc::new(move |processed_count, total_count, msg| {
            let mut p = p.lock().unwrap();
            p[idx] = (processed_count, total_count);

            let processed_count = p.iter().map(|v| v.0).sum();
            let total_count = p.iter().map(|v| v.1).sum();
            inner.report(processed_count, total_count, msg)
        }))
    }
}

struct IndicatifProgressReporter {
    bar: ProgressBar,
}

impl Default for IndicatifProgressReporter {
    fn default() -> Self {
        let bar = ProgressBar::new(0);
        bar.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}"),
        );
        Self { bar }
    }
}

impl IndicatifProgressReporter {
    pub fn get_reporter(&self) -> ProgressReporter {
        let bar = self.bar.clone();
        ProgressReporter::new(Arc::new(move |processed_count, total_count, text: &str| {
            if bar.length() != total_count {
                bar.set_length(total_count);
            }
            if bar.position() != processed_count {
                bar.set_position(processed_count);
            }
            bar.set_message(text);
        }))
    }

    pub fn finish(&self) {
        self.bar.finish();
    }
}
