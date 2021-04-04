mod checked_path;
mod db;
mod pdf_to_images;
mod progress;
mod utils;
mod video_exts;
mod web;

use anyhow::{Context, Result};
use checked_path::{CheckedPath, Kind};
use db::{Db, DbPool};
use dialoguer::Confirm;
use matching::ImageVideoMatcher;
use matching_opencv::OpenCVImageVideoMatcher;
use pdf_to_images::pdfs_to_images;
use progress::{ComposedProgressReporter, IndicatifProgressReporter};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::collections::HashSet;
use std::path::PathBuf;
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
    #[structopt(long, short = "n")]
    non_interactive: bool,
}

#[async_std::main]
async fn main() -> Result<()> {
    let opt: Opt = Opt::from_args();

    let db_pool = DbPool::connect().await?;
    let mut db = db_pool.db().await?;

    let (pdfs, videos) = process_files(&opt.files, &mut db).await?;
    let videos_to_process = get_videos_to_process(&videos, &pdfs, &opt, &mut db).await?;

    let reporter = IndicatifProgressReporter::default();
    let pages = pdfs_to_images(
        &pdfs.iter().map(|p| p).collect(),
        &db_pool,
        reporter.get_reporter(),
    )?;
    reporter.finish();

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

    for (video, task) in tasks {
        let matchings = task.process();
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

async fn process_files(
    files: &Vec<PathBuf>,
    db: &mut Db<'static>,
) -> Result<(Vec<HashedFile>, Vec<HashedFile>)> {
    let paths = get_files_with_hash(files)?;

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

    Ok((pdfs, videos))
}

fn get_files_with_hash(files: &Vec<PathBuf>) -> Result<Vec<CheckedPath>> {
    let paths = files
        .iter()
        .cloned()
        .map(CheckedPath::from)
        .collect::<Result<Vec<CheckedPath>>>()?;

    Ok(paths
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
        .collect::<Result<Vec<CheckedPath>>>()?)
}

async fn get_videos_to_process<'a>(
    videos: &'a Vec<HashedFile>,
    pdfs: &Vec<HashedFile>,
    opt: &Opt,
    db: &mut Db<'static>,
) -> Result<Vec<&'a HashedFile>> {
    let pdf_hashes: HashSet<&str> = pdfs.iter().map(|p| &p.hash as &str).collect();
    let mut videos_to_process = Vec::new();
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
    Ok(videos_to_process)
}
