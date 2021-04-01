use std::path::Path;
use std::time::Duration;

pub trait ImageVideoMatcher<P: ProgressReporter + Copy> {
    fn create_video_matcher<'i, I: MatchableImage + Send + Sync + Copy + Eq + 'i>(
        &self,
        images: Vec<I>,
        progress_reporter: P,
    ) -> Box<dyn VideoMatcher<I, P> + 'i>;
}

pub trait VideoMatcher<I: MatchableImage + Clone, P: ProgressReporter + Copy> {
    fn match_images_with_video(&self, video_path: &Path, progress_reporter: P) -> Vec<Matching<I>>;
}

pub trait MatchableImage {
    fn get_path(&self) -> &Path;
}

pub trait ProgressReporter: Sized + Send {
    fn report(&self, progress: f32);
}

#[derive(Clone)]
pub struct Matching<I: Clone> {
    pub video_time: Duration,
    pub video_frame_idx: usize,
    pub image: Option<I>,
}
