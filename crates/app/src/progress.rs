use indicatif::{ProgressBar, ProgressStyle};
use matching::ProgressReporter;
use std::sync::{Arc, Mutex};

pub struct ComposedProgressReporter {
    progress: Arc<Mutex<Vec<(u64, u64)>>>,
    inner: ProgressReporter,
}

impl ComposedProgressReporter {
    pub fn new(inner: ProgressReporter) -> Self {
        ComposedProgressReporter {
            progress: Arc::new(Mutex::new(Vec::new())),
            inner,
        }
    }
}

impl ComposedProgressReporter {
    pub fn create_nested(&self) -> ProgressReporter {
        let mut p = self.progress.lock().unwrap();
        let idx = p.len();
        p.push((0, 0));
        let p = self.progress.clone();
        let inner = self.inner.clone();

        ProgressReporter::new(Arc::new(move |processed_count, total_count, msg| {
            let mut p = p.lock().unwrap();
            p[idx] = (processed_count, total_count);

            let processed_count = p.iter().map(|v| v.0).sum();
            let total_count = p.iter().map(|v| v.1).sum();
            inner.report(processed_count, total_count, msg)
        }))
    }
}

pub struct IndicatifProgressReporter {
    bar: ProgressBar,
}

impl Default for IndicatifProgressReporter {
    fn default() -> Self {
        let bar = ProgressBar::new(0);
        bar.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}"),
        );
        Self { bar }
    }
}

impl IndicatifProgressReporter {
    pub fn get_reporter(&self) -> ProgressReporter {
        let bar = self.bar.clone();
        ProgressReporter::new(Arc::new(move |processed_count, total_count, text: &str| {
            if bar.length() != total_count {
                bar.set_length(total_count);
            }
            if bar.position() != processed_count {
                bar.set_position(processed_count);
            }
            bar.set_message(text);
        }))
    }

    pub fn finish(&self) {
        self.bar.finish();
    }
}
