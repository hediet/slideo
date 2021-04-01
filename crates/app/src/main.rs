#![feature(proc_macro_hygiene, decl_macro)]

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
use matching::{ImageVideoMatcher, MatchableImage, Matching, ProgressReporter};
use matching_opencv::OpenCVImageVideoMatcher;
use pdf_to_images::{pdfs_to_images, PdfPage};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use structopt::StructOpt;
use utils::hash_file;
use web::start_server;

#[derive(StructOpt, Debug)]
#[structopt(name = "slideo")]
struct Opt {
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

    let pages = pdfs_to_images(&pdfs.iter().map(|p| p).collect(), &db_pool)?;

    let mut tx = db.begin_trans().await?;
    for video in &videos_to_process {
        //let mapping_info = tx.find_mapping_info(&video.hash).await?;
        tx.create_or_reset_video(&video.hash, pdfs.iter().map(|v| &v.hash as &str))
            .await?;
    }
    tx.commit().await?;

    let m = OpenCVImageVideoMatcher::default();

    let reporter = ConsoleProgressReporter::new();

    let video_matcher = m.create_video_matcher(pages.iter().collect(), &reporter);

    for video in &videos_to_process {
        let matchings: Vec<Matching<&PdfPage>> =
            video_matcher.match_images_with_video(&video.path, &reporter);

        let mut tx = db.begin_trans().await?;
        tx.update_video_matchings(&video.hash, matchings.iter())
            .await?;
        tx.commit().await?;
    }

    let first = pdfs.iter().next();

    if !opt.non_interactive {
        start_server(first.map(|h| h.hash.clone()))?;
    }

    Ok(())
}

impl<'a> MatchableImage for &PdfPage<'a> {
    fn get_path(&self) -> &Path {
        &self.image_path
    }
}

struct ConsoleProgressReporter {}

impl ConsoleProgressReporter {
    pub fn new() -> ConsoleProgressReporter {
        ConsoleProgressReporter {}
    }
}

impl ProgressReporter for &ConsoleProgressReporter {
    fn report(&self, progress: f32) {
        println!("{}%", progress * 100.0);
    }
}
