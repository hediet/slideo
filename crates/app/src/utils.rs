use anyhow::Result;
use std::{
    fs::File,
    io::copy,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

pub fn get_temp_path() -> PathBuf {
    let temp_dir = std::env::temp_dir();
    let f = temp_dir.join(&"pdf-video-sync");
    return f;
}

fn hash(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value);
    let result = hasher.finalize();
    let hash_str = format!("{:x}", result);
    return hash_str;
}

pub fn get_temp_path_key(category: &str, key: &str) -> PathBuf {
    return get_temp_path().join(&format!("{}-{}", category, &hash(key)[0..20]));
}

pub fn hash_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut sha256 = Sha256::new();
    copy(&mut file, &mut sha256)?;
    Ok(format!("{:x}", sha256.finalize()))
}
