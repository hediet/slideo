use std::sync::Arc;

#[derive(Clone)]
pub struct ProgressReporter {
    handler: Arc<dyn Fn(u64, u64, &str) + Sync + Send>,
}

impl ProgressReporter {
    pub fn new(handler: Arc<dyn Fn(u64, u64, &str) + Sync + Send>) -> Self {
        Self { handler }
    }

    pub fn report(&self, processed_count: u64, total_count: u64, message: &str) {
        let h = &self.handler;
        h(processed_count, total_count, message);
    }
}
