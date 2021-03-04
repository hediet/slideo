use std::{
    path::{Path, PathBuf},
    process::Command,
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

pub fn get_temp_path_key(key: &str) -> PathBuf {
    return get_temp_path().join(&hash(key)[0..20]);
}

pub fn pdf_to_images(pdf: &Path) -> Vec<PathBuf> {
    let path = get_temp_path_key(&format!("slides-v1-{}", &pdf.to_str().unwrap()));

    if !path.exists() {
        std::fs::create_dir_all(&path).unwrap();

        let result = Command::new(&"pdftocairo")
            .arg(&pdf)
            .arg(&"-png")
            .arg(&path.join("page"))
            .status()
            .expect(&format!("Cairo should extract pdf slides"));
        if !result.success() {
            panic!("Cairo failed")
        }
    }

    return glob::glob(&path.join(&"*.png").to_string_lossy())
        .unwrap()
        .map(|p| p.unwrap())
        .collect();
}
