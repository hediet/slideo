use crate::{
    db::{DbPool, PdfExtractedPagesDir},
    utils::get_temp_path_key,
    HashedFile,
};
use anyhow::Result;
use async_std::task::block_on;
use lexical_sort::{natural_lexical_cmp, PathSort};
use matching::{MatchableImage, ProgressReporter};
use pdftocairo::pdf_info;
use rand::Rng;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Debug, Eq, PartialEq)]
pub struct PdfPage<'t> {
    pub pdf_path: &'t Path,
    pub pdf_hash: &'t str,
    pub image_path: PathBuf,
    pub page_idx: usize,
}

impl<'a> MatchableImage for &PdfPage<'a> {
    fn get_path(&self) -> &Path {
        &self.image_path
    }
}

pub fn pdfs_to_images<'t>(
    pdf_files: &Vec<&'t HashedFile>,
    db_pool: &DbPool,
    progress_reporter: ProgressReporter,
) -> Result<Vec<PdfPage<'t>>> {
    let mut pdf_files = pdf_files.clone();
    pdf_files.dedup_by_key(|p| &p.hash); // Remove duplicated pdf files

    let total_page_count: u32 = pdf_files
        .par_iter()
        .map(|f| pdf_info(&f.path).unwrap().page_count())
        .sum();

    let progresses = Arc::new(Mutex::new(HashMap::new()));

    let result: Result<Vec<Vec<PdfPage<'t>>>> = pdf_files
        .par_iter()
        .map(|f| -> Result<Vec<PdfPage<'t>>> {
            let mut db = block_on(db_pool.db())?;
            let mut tx = block_on(db.begin_trans())?;
            let result: Option<PdfExtractedPagesDir> =
                block_on(tx.get_pdf_extracted_pages_dir(&f.hash))?;

            let (target_dir, finished) = match result {
                Some(data) if data.finished => (data.dir, true),
                _ => {
                    let mut rng = rand::thread_rng();
                    let rand_idx: u128 = rng.gen();
                    (
                        get_temp_path_key("slides", &format!("{}-{:?}", &f.hash, rand_idx)),
                        false,
                    )
                }
            };

            if !finished {
                block_on(tx.set_pdf_extracted_pages_dir(&PdfExtractedPagesDir {
                    dir: target_dir.clone(),
                    finished: false,
                    pdf_hash: f.hash.clone(),
                }))?;
            }

            block_on(tx.commit())?;

            let pages = pdf_to_images(&f.hash, &f.path, &target_dir, |processed_pages| {
                let mut map = progresses.lock().unwrap();
                map.insert(&f.hash, processed_pages);
                let total_processed_pages: u32 = map.values().sum();
                progress_reporter.report(
                    total_processed_pages as u64,
                    total_page_count as u64,
                    "Extracting PDF pages...",
                );
            })?;

            if !finished {
                let mut tx = block_on(db.begin_trans())?;
                block_on(tx.set_pdf_extracted_pages_dir(&PdfExtractedPagesDir {
                    dir: target_dir.clone(),
                    finished: true,
                    pdf_hash: f.hash.clone(),
                }))?;
                block_on(tx.commit())?;
            }

            Ok(pages)
        })
        .collect();

    progress_reporter.report(
        total_page_count as u64,
        total_page_count as u64,
        "PDF extraction successful.",
    );

    let flatten = result.into_iter().flatten().flatten().collect();
    Ok(flatten)
}

fn pdf_to_images<'t>(
    pdf_hash: &'t str,
    pdf_path: &'t Path,
    target_dir: &Path,
    progress: impl Fn(u32),
) -> Result<Vec<PdfPage<'t>>> {
    /*
    if target_dir.exists() {
        println!("Removing {:?}", target_dir);
        std::fs::remove_dir_all(target_dir).unwrap();
    }
    */

    if !target_dir.exists() {
        pdftocairo::pdftocairo(
            pdf_path,
            target_dir,
            pdftocairo::Options {
                progress: Some(|p: pdftocairo::ProgressInfo| {
                    progress(p.processed_pages);
                }),
                ..pdftocairo::Options::default()
            },
        )?;
    }

    let mut vec: Vec<PathBuf> = glob::glob(&target_dir.join(&"*.png").to_string_lossy())
        .unwrap()
        .map(|p| p.unwrap())
        .collect();
    vec.path_sort(natural_lexical_cmp);

    progress(vec.len() as u32);

    Ok(vec
        .into_iter()
        .enumerate()
        .map(|(page_idx, image_path)| PdfPage {
            page_idx,
            image_path,
            pdf_path,
            pdf_hash,
        })
        .collect())
}
