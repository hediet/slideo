use std::{collections::HashMap, io, path::Path, process::Command};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PdfInfo {
    page_count: u32,
}

impl PdfInfo {
    /// Returns the page count.
    pub fn page_count(&self) -> u32 {
        self.page_count
    }
}

/// Invokes the `pdfinfo` tool and parses the result.
pub fn pdf_info(pdf: &Path) -> Result<PdfInfo, io::Error> {
    let result = Command::new(&"pdfinfo").arg(&pdf).output()?;

    if !result.status.success() {
        println!("unsuccess");
    }

    let stdout = String::from_utf8_lossy(&result.stdout);
    let info = parse_pdf_info(&stdout);
    let page_count: u32 = info["Pages"].parse().unwrap();

    Ok(PdfInfo { page_count })
}

fn parse_pdf_info(lines: &str) -> HashMap<String, String> {
    let mut map = HashMap::<String, String>::new();

    for line in lines.split('\n') {
        if line.trim().len() == 0 {
            continue;
        }

        let parts: Vec<&str> = line.splitn(2, ':').collect();
        if parts.len() != 2 {
            panic!("Got invalid line '{}'", line);
        }
        map.insert(parts[0].trim().to_owned(), parts[1].trim().to_owned());
    }

    map
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    #[test]
    fn test_pdf_info_works() {
        let path = PathBuf::from("../../data/video/slides.pdf");
        let info = super::pdf_info(&path).unwrap();
        assert_eq!(info.page_count(), 42);
    }
}
