mod progress;
pub use progress::*;

use std::path::Path;
use std::time::Duration;

pub trait ImageVideoMatcher<'i> {
    /// Prepares a video matcher for a given set of images.
    fn create_video_matcher<I: MatchableImage + Send + Sync + Copy + Eq + 'i>(
        &self,
        images: Vec<I>,
        progress_reporter: ProgressReporter,
    ) -> Box<dyn VideoMatcher<'i, I> + 'i>;
}

pub trait VideoMatcher<'i, I: MatchableImage + Clone + 'i> {
    /// Creates a match task for a given video with a given progress reporter.
    /// Reports immediate progress.
    fn match_images_with_video(
        &self,
        video_path: &Path,
        progress_reporter: ProgressReporter,
    ) -> Box<dyn VideoMatcherTask<I> + 'i>;
}

pub trait VideoMatcherTask<I: MatchableImage + Clone> {
    /// Computes the matchings.
    fn process(&self) -> Vec<Matching<I>>;
}

pub trait MatchableImage {
    fn get_path(&self) -> &Path;
}

#[derive(Clone)]
pub struct Matching<I: Clone> {
    pub video_time: Duration,
    pub video_frame_idx: usize,
    pub image: Option<I>,
}

/*
mod Bla {
    use std::sync::{RwLock, Weak};


    let task = TaskDef::new("main");

    let step1 = task.new_subtask("step1");
    let step2 = task.new_subtask("step2");

    task.add_change_handler()

    */
/*
    struct TaskManager {
        next_id: u64,

    }

    struct TaskInfo {}

    impl TaskManager {
        pub fn new_task(&self, name: String) -> TaskDef {

        }
    }

    struct TaskDef {}

    impl TaskDef {
        pub fn new_subtask(&self, name: String) -> TaskDef {}
    }


    struct TaskRef {}

    impl TaskRef {

    }

    struct Task {
        pub sub_tasks: Arc<RwLock<Vec<Weak<Task>>>>,
        pub parent_task: Arc<Task>,
    }

    impl Task {
        pub fn add_sub_task(&self) -> Task {}

        pub fn report_progress(&self, processed_count: u64, total_count: u64) {}

        fn handle_sub_task_progress() {}
    }

}

*/
