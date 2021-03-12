use crate::video_exts::is_video_ext;
use anyhow::{anyhow, Result};
use std::path::PathBuf;

#[derive(Debug, Eq, PartialEq)]
pub enum Kind {
    Pdf,
    Video,
}

#[derive(Debug)]
pub struct CheckedPath {
    pub path: PathBuf,
    pub kind: Kind,
    pub hash: Option<String>,
}

impl CheckedPath {
    pub fn from(path: PathBuf) -> Result<CheckedPath> {
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
            } else if is_video_ext(&ext_str) {
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
    }
}
