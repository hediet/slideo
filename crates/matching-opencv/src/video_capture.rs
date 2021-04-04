use opencv::{
    prelude::*,
    videoio::{VideoCapture, CAP_PROP_FPS, CAP_PROP_FRAME_COUNT, CAP_PROP_POS_FRAMES},
};
use std::{path::Path, rc::Rc, time::Duration};

use super::image_utils::{compute_similarity, to_small_image};

pub struct VideoCaptureIter {
    video: VideoCapture,
    fps: f64,
    interval: Duration,
}

impl VideoCaptureIter {
    pub fn open(path: &Path, interval: Duration) -> Self {
        let video = opencv::videoio::VideoCapture::from_file(
            &path.to_string_lossy(),
            0, //CAP_DSHOW
        )
        .unwrap();
        let fps = video.get(CAP_PROP_FPS).unwrap();
        VideoCaptureIter {
            video,
            fps,
            interval,
        }
    }

    pub fn total_frames(&self) -> f64 {
        self.video.get(CAP_PROP_FRAME_COUNT).unwrap()
    }

    pub fn total_time(&self) -> Duration {
        Duration::from_secs_f64(self.video.get(CAP_PROP_FRAME_COUNT).unwrap() / self.fps)
    }
}

impl Iterator for VideoCaptureIter {
    type Item = (Mat, Duration, usize);

    fn next(&mut self) -> Option<(Mat, Duration, /* frame */ usize)> {
        let mut frame = Mat::default().unwrap();
        loop {
            let frame_idx = self.video.get(CAP_PROP_POS_FRAMES).unwrap();
            let time_passed = Duration::from_secs_f64(frame_idx / self.fps);

            if !self.video.grab().unwrap() {
                return None;
            }

            if frame_idx % (self.fps * self.interval.as_secs_f64()).floor() < 1.0 {
                self.video.retrieve(&mut frame, 0).unwrap();
                return Some((frame, time_passed, frame_idx as usize));
            }
        }
    }
}

pub struct MarkSimilarIter<I>
where
    I: Iterator<Item = (Mat, Duration, usize)>,
{
    iter: I,
    last_frame: Option<Rc<Mat>>,
}

impl<I> MarkSimilarIter<I>
where
    I: Iterator<Item = (Mat, Duration, usize)>,
{
    pub fn new(iter: I) -> Self {
        return MarkSimilarIter {
            iter,
            last_frame: None,
        };
    }
}

impl<I> Iterator for MarkSimilarIter<I>
where
    I: Iterator<Item = (Mat, Duration, usize)>,
{
    type Item = (bool, Mat, Duration, usize);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some((frame, frame_time, frame_idx)) = self.iter.next() {
            let scaled_frame = to_small_image(&frame);
            let similarity = if let Some(last_frame) = &self.last_frame {
                compute_similarity(last_frame, &scaled_frame)
            } else {
                0.0
            };

            let rc = Rc::new(scaled_frame);

            self.last_frame = Some(rc.clone());
            Some((similarity < 0.98, frame, frame_time, frame_idx))
        } else {
            None
        }
    }
}
