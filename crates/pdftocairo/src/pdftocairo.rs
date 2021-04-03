use std::{
    fs::create_dir_all,
    io,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use crate::pdf_info;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Page {
    pub index: u32,
    pub image_path: PathBuf,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ProgressInfo {
    pub total_pages: u32,
    pub processed_pages: u32,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Format {
    Png,
    Jpeg,
    Pdf,
    Svg,
    Eps,
}

impl Format {
    fn to_args(&self) -> Vec<String> {
        match self {
            Format::Png => vec!["-png".to_owned()],
            Format::Jpeg => vec!["-jpeg".to_owned()],
            Format::Pdf => vec!["-pdf".to_owned()],
            Format::Svg => vec!["-svg".to_owned()],
            Format::Eps => vec!["-eps".to_owned()],
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Color {
    Color,
    Mono,
    Gray,
}

impl Color {
    fn to_args(&self) -> Vec<String> {
        match self {
            Color::Color => vec![],
            Color::Mono => vec!["-mono".to_owned()],
            Color::Gray => vec!["-gray".to_owned()],
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Pages {
    All,
    Odd,
    Even,
}

impl Pages {
    fn to_args(&self) -> Vec<String> {
        match self {
            Pages::All => vec![],
            Pages::Odd => vec!["-o".to_owned()],
            Pages::Even => vec!["-e".to_owned()],
        }
    }
}

pub struct Options<P> {
    pub format: Format,
    pub color: Color,
    pub pages: Pages,
    /// One based
    pub first_page: Option<u32>,
    /// One based
    pub last_page: Option<u32>,
    pub progress: Option<P>,
}

impl<P> Options<P> {
    fn exported_page_count(&self, pdf_page_count: u32) -> u32 {
        let mut count = 0;
        let first_page = self.first_page.unwrap_or(1);
        let last_page = self.last_page.unwrap_or(pdf_page_count);

        for i in 1..(pdf_page_count + 1) {
            match self.pages {
                Pages::All => {}
                Pages::Odd => {
                    // this might be wrong
                    if i % 2 == 1 {
                        continue;
                    }
                }
                Pages::Even => {
                    if i % 2 == 0 {
                        continue;
                    }
                }
            }
            if i < first_page {
                continue;
            }
            if i > last_page {
                continue;
            }
            count += 1;
        }
        count
    }
}

impl<P> Default for Options<P> {
    fn default() -> Self {
        Options {
            format: Format::Png,
            color: Color::Color,
            first_page: None,
            last_page: None,
            pages: Pages::All,
            progress: None,
        }
    }
}

pub fn pdftocairo<P: Fn(ProgressInfo)>(
    pdf: &Path,
    target_dir: &Path,
    options: Options<P>,
) -> Result<Vec<Page>, io::Error> {
    if !target_dir.exists() {
        create_dir_all(target_dir)?;
    }

    let has_items = target_dir.read_dir()?.next().is_some();
    if has_items {
        panic!("The given target directory must be empty!");
    }

    let pdf_info = pdf_info(pdf)?;

    let result = Arc::new(Mutex::<Option<()>>::new(None));

    let pdf2 = pdf.to_owned();
    let target_dir2 = target_dir.to_owned();
    let result2 = result.clone();

    let mut cmd = Command::new(&"pdftocairo");
    cmd.arg(&pdf2);
    cmd.arg(&target_dir2.join("p"));
    cmd.args(&options.color.to_args());
    cmd.args(&options.format.to_args());
    cmd.args(&options.pages.to_args());
    if let Some(f) = options.first_page {
        cmd.args(vec!["-f", &f.to_string()]);
    }
    if let Some(l) = options.last_page {
        cmd.args(vec!["-l", &l.to_string()]);
    }

    thread::spawn(move || {
        let r = cmd
            .status()
            .expect(&format!("Cairo should extract pdf slides"));
        if !r.success() {
            // TODO
        }
        let mut m = result2.lock().unwrap();
        *m = Some(());
    });

    let page_count = options.exported_page_count(pdf_info.page_count());

    let mut last = ProgressInfo {
        total_pages: 0,
        processed_pages: 0,
    };
    let mut report_progress = |p: u32| {
        let new = ProgressInfo {
            total_pages: page_count,
            processed_pages: p,
        };
        if new != last {
            if let Some(p) = &options.progress {
                p(new.clone());
            }
            last = new;
        }
    };

    while result.lock().unwrap().is_none() {
        report_progress(target_dir.read_dir()?.count() as u32);

        thread::sleep(Duration::from_millis(500));
    }

    report_progress(target_dir.read_dir()?.count() as u32);

    let mut result = Vec::<Page>::new();

    for item in target_dir.read_dir()? {
        let item = item?;
        let file_name = item.file_name(); // e.g. p-01.png
        let file_name = file_name.to_string_lossy();

        let name_without_ext = file_name.split('.').next().unwrap();
        let page_idx_str = &name_without_ext[2..];
        let page_idx: u32 = page_idx_str.parse().unwrap();
        result.push(Page {
            image_path: item.path(),
            index: page_idx,
        });
    }

    result.sort_by_key(|r| r.index);

    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use tempdir::TempDir;

    use super::*;
    use crate::Options;

    #[ignore]
    #[test]
    fn test_pdftocairo() {
        let path = PathBuf::from("../../data/video/slides.pdf");
        let temp_dir = TempDir::new("pdftocairo_test").unwrap();
        let pages = pdftocairo(
            &path,
            temp_dir.path(),
            Options {
                first_page: Some(1),
                last_page: Some(8),
                pages: Pages::Even,
                progress: Some(|info| {
                    println!("{:?}", info);
                }),
                ..Options::default()
            },
        )
        .unwrap();

        assert_eq!(
            pages.iter().map(|p| p.index).collect::<Vec<_>>(),
            vec![1, 3, 5, 7]
        );
    }
}
