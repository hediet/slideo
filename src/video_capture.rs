use std::{path::PathBuf, rc::Rc};

use opencv::{
    core::Size,
    imgproc::{resize, INTER_AREA},
    prelude::*,
    videoio::{VideoCapture, CAP_PROP_FPS, CAP_PROP_FRAME_COUNT, CAP_PROP_POS_FRAMES},
};

use crate::image_utils::{get_similarity, to_small_image};

pub struct VideoCaptureIter {
    video: VideoCapture,
    fps: f64,
    interval_s: f64,
}

impl VideoCaptureIter {
    pub fn open(path_buf: &PathBuf, interval_s: f64) -> Self {
        let video = opencv::videoio::VideoCapture::from_file(
            &path_buf.to_string_lossy(),
            0, //CAP_DSHOW
        )
        .unwrap();
        let fps = video.get(CAP_PROP_FPS).unwrap();
        return VideoCaptureIter {
            video,
            fps,
            interval_s,
        };
    }

    pub fn total_frames(&self) -> f64 {
        return self.video.get(CAP_PROP_FRAME_COUNT).unwrap();
    }
}

impl Iterator for VideoCaptureIter {
    type Item = (f64, Mat);

    fn next(&mut self) -> Option<(f64, Mat)> {
        let mut frame = Mat::default().unwrap();
        loop {
            let frame_idx = self.video.get(CAP_PROP_POS_FRAMES).unwrap();

            if !self.video.grab().unwrap() {
                return None;
            }

            if (frame_idx / self.fps > 110.0)
                && (frame_idx % (self.fps * self.interval_s).floor() < 1.0)
            {
                self.video.retrieve(&mut frame, 0).unwrap();
                return Some((frame_idx, frame));
            }
        }
    }
}

pub struct FilterIter<I>
where
    I: Iterator<Item = (f64, Mat)>,
{
    iter: I,
    last_frame: Option<Rc<Mat>>,
}

impl<I> FilterIter<I>
where
    I: Iterator<Item = (f64, Mat)>,
{
    pub fn new(iter: I) -> Self {
        return FilterIter {
            iter,
            last_frame: None,
        };
    }
}

impl<I> Iterator for FilterIter<I>
where
    I: Iterator<Item = (f64, Mat)>,
{
    type Item = (f64, Mat, Rc<Mat>);

    fn next(&mut self) -> Option<(f64, Mat, Rc<Mat>)> {
        loop {
            if let Some((frame_id, frame)) = self.iter.next() {
                let scaled_frame = to_small_image(&frame);
                let similarity = if let Some(last_frame) = &self.last_frame {
                    get_similarity(last_frame, &scaled_frame)
                } else {
                    0.0
                };

                let rc = Rc::new(scaled_frame);
                self.last_frame = Some(rc.clone());

                if similarity < 0.98 {
                    return Some((frame_id, frame, rc));
                }
            } else {
                return None;
            }
        }
    }
}
