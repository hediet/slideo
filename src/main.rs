//mod migrations;
mod db;
mod video_exts;

use anyhow::{Context, Result};
use db::{Db, HashedFile};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
/*use migrations::CreateUsers;
use rusqlite::Connection;
use schemamama::Migrator;
use schemamama_rusqlite::SqliteAdapter;*/
use sha2::{Digest, Sha256};

use anyhow::anyhow;
use std::{borrow::Borrow, cell::RefCell, collections::HashSet, path::PathBuf, rc::Rc};
use std::{ffi::OsString, path::Path};
use std::{fs::File, io};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "slideo")]
struct Opt {
    #[structopt(name = "FILES", parse(from_os_str), required = true)]
    files: Vec<PathBuf>,

    #[structopt(long)]
    invalidate_cache: bool,

    #[structopt(long)]
    ignore_video_ext: bool,

    #[structopt(long)]
    non_interactive: bool,
}

#[async_std::main]
async fn main() -> Result<()> {
    let opt: Opt = Opt::from_args();

    #[derive(Debug, Eq, PartialEq)]
    enum Kind {
        Pdf,
        Video,
    }

    #[derive(Debug)]
    struct CheckedPath {
        path: PathBuf,
        kind: Kind,
        hash: Option<String>,
    }

    let ext_set: HashSet<String> = video_exts::get_video_exts().into_iter().collect();

    let paths = opt
        .files
        .into_iter()
        .map(|path| {
            if path.is_dir() {
                return Err(anyhow!(
                    "The path '{}' is a directory, but a file was expected!",
                    path.to_string_lossy()
                ));
            }

            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy();
                if ext_str == "pdf" {
                    Ok(CheckedPath {
                        path,
                        kind: Kind::Pdf,
                        hash: None,
                    })
                } else if ext_set.contains::<str>(&ext_str) {
                    Ok(CheckedPath {
                        path,
                        kind: Kind::Video,
                        hash: None,
                    })
                } else {
                    Err(anyhow!(
                        "Unsupported file extension '{}' in path '{}'!",
                        ext_str,
                        path.to_string_lossy()
                    ))
                }
            } else {
                Err(anyhow!(
                    "Unsupported file extension in path '{}'!",
                    path.to_string_lossy()
                ))
            }
        })
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

    let mut db = Db::connect().await?;

    db.update_hashes(
        paths
            .iter()
            .map(|p| HashedFile::new(p.path.clone(), p.hash.clone().unwrap())),
    )
    .await?;

    let videos = paths
        .iter()
        .filter(|p| p.kind == Kind::Video)
        .map(|p| HashedFile::new(p.path.clone(), p.hash.clone().unwrap()))
        .collect::<Vec<_>>();

    let pdfs = paths
        .iter()
        .filter(|p| p.kind == Kind::Pdf)
        .map(|p| HashedFile::new(p.path.clone(), p.hash.clone().unwrap()))
        .collect::<Vec<_>>();

    /*
        for video in &videos {
            let mapping_info = db.find_mapping_info(video).await?;

            if let Some(existing) = mapping_info.existing_mapping_info {}
        }

        for video in &videos {
            let mut mapping_info = db.find_mapping_info(video).await?;
            mapping_info.create_or_reset().await?;
        }
    */
    Ok(())
}

// slideo foo.pdf
// No known video files!

// slideo foo.pdf vorlesung1.mp4
// processing...
// opens video

// slideo foo.pdf baz.pdf vorlesung1.mp4 vorlesung2.mp4
// clears cache of vorlesung1.mp4 and vorlesung2.mp4 Proceed? Yes
// processing...

// slideo *.pdf *.mp4
// processing...
// Select a single pdf file to view!

/*
{
    pdfs: [{ sha265: "123" }, { sha265: "123" }],
    videos: [
        {
            sha265: "123",
            mappings: [
                { videoIdx: 0, offsetMs: 1200, pdfIdx: 0, page: 10 },
                { videoIdx: 0, offsetMs: 1200 },
            ]
        }
    ]
}

*/

pub fn hash_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut sha256 = Sha256::new();
    io::copy(&mut file, &mut sha256)?;
    Ok(format!("{:x}", sha256.finalize()))
}
