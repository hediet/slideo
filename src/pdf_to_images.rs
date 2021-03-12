use crate::{
    db::{DbPool, PdfExtractedPagesDir},
    utils::get_temp_path_key,
    HashedFile,
};
use anyhow::{anyhow, Result};
use async_std::task::block_on;
use lexical_sort::{natural_lexical_cmp, PathSort};
use rand::Rng;
use std::{
    fs::create_dir_all,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, Eq, PartialEq)]
pub struct PdfPage<'t> {
    pub pdf_path: &'t Path,
    pub pdf_hash: &'t str,
    pub image_path: PathBuf,
    pub page_idx: usize,
}

pub fn pdfs_to_images<'t>(
    pdf_files: &Vec<&'t HashedFile>,
    db_pool: &DbPool,
) -> Result<Vec<PdfPage<'t>>> {
    print!("Extracting pdf pages...");

    let result: Result<Vec<Vec<PdfPage<'t>>>> = pdf_files
        .into_iter()
        .map(|f| -> Result<Vec<PdfPage<'t>>> {
            println!(
                "Processing pdf '{}', hash '{}'",
                f.path.to_string_lossy(),
                f.hash
            );
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

            let pages = pdf_to_images(&f.hash, &f.path, &target_dir)?;

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

    let flatten = result.into_iter().flatten().flatten().collect();
    println!(" Finished!");
    Ok(flatten)
}

fn pdf_to_images<'t>(
    pdf_hash: &'t str,
    pdf_path: &'t Path,
    target_dir: &Path,
) -> Result<Vec<PdfPage<'t>>> {
    if !target_dir.exists() {
        create_dir_all(&target_dir).unwrap();

        let result = Command::new(&"pdftocairo")
            .arg(&pdf_path)
            .arg(&"-png")
            .arg(&target_dir.join("page"))
            .status()
            .expect(&format!("Cairo should extract pdf slides"));
        if !result.success() {
            return Err(anyhow!("Cairo failed"));
        }
    }

    let mut vec: Vec<PathBuf> = glob::glob(&target_dir.join(&"*.png").to_string_lossy())
        .unwrap()
        .map(|p| p.unwrap())
        .collect();
    vec.path_sort(natural_lexical_cmp);

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
