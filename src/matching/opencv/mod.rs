mod feature_extractor;
mod flann;
mod image_utils;
mod video_capture;

use self::{
    flann::FlannMatcher,
    image_utils::{get_similarity, to_small_image, Transformation2D},
};
use super::{ImageVideoMatcher, MatchableImage, Matching, ProgressReporter, VideoMatcher};
use feature_extractor::FeatureExtractor;
use opencv::{
    core::{KeyPoint, Scalar},
    //highgui::{imshow, wait_key},
    imgcodecs::*,
    imgproc::{cvt_color, warp_affine, COLOR_BGRA2BGR, WARP_INVERSE_MAP},
    prelude::*,
};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use std::{cell::RefCell, sync::Mutex};
use thread_local::ThreadLocal;
use video_capture::{FilterIter, VideoCaptureIter};

#[derive(Default)]
pub struct OpenCVImageVideoMatcher {}

impl OpenCVImageVideoMatcher {
    fn create_video_matcher<
        'i,
        I: MatchableImage + Send + Sync + Copy + Eq + 'i,
        P: ProgressReporter + Copy,
    >(
        &self,
        images: Vec<I>,
        _progress_reporter: P,
    ) -> OpenCVVideoMatcher<I> {
        let processed_images: Vec<ProcessedImage<I>> =
            images.into_iter().map(ProcessedImage::compute).collect();

        OpenCVVideoMatcher {
            shared_flanns: Arc::new(ThreadLocal::new()),
            images: processed_images,
        }
    }
}

impl<P: ProgressReporter + Copy + Send + Sync> ImageVideoMatcher<P> for OpenCVImageVideoMatcher {
    fn create_video_matcher<'i, I: MatchableImage + Send + Sync + Copy + Eq + 'i>(
        &self,
        images: Vec<I>,
        progress_reporter: P,
    ) -> Box<dyn VideoMatcher<I, P> + 'i> {
        Box::new(self.create_video_matcher(images, progress_reporter))
    }
}

struct ProcessedImage<I> {
    pub source_img: I,
    pub keypoints: Vec<KeyPoint>,
    pub descriptors: Mat,
    pub img: Mat,
    pub small_img: Mat,
}

unsafe impl<I: Send> Send for ProcessedImage<I> {}
unsafe impl<I: Send> Sync for ProcessedImage<I> {}

thread_local! {
    static FEATURE_EXTRACTOR: RefCell<FeatureExtractor> = RefCell::new(FeatureExtractor::default());
}

impl<I: MatchableImage> ProcessedImage<I> {
    pub fn compute(img: I) -> ProcessedImage<I> {
        let path = img.get_path();
        if !path.exists() {
            panic!("File '{:?}' must exist", path);
        }
        let img_mat = imread(&path.to_string_lossy(), 0).unwrap();
        if img_mat.empty().unwrap() {
            panic!("Could not read file '{:?}'", path);
        }

        let mut normalized_img_mat = Mat::default().unwrap();
        cvt_color(&img_mat, &mut normalized_img_mat, COLOR_BGRA2BGR, 0).unwrap();

        let result = FEATURE_EXTRACTOR.with(|e| {
            e.borrow_mut()
                .find_keypoints_and_descriptors(&normalized_img_mat)
        });

        /*
        let mut debug_keypoints = Mat::default().unwrap();
        draw_keypoints(
            &normalized_img_mat,
            &result.keypoints,
            &mut debug_keypoints,
            Scalar::new(255.0, 0.0, 0.0, 0.0),
            DrawMatchesFlags::DEFAULT,
        )
        .unwrap();
        imshow(&"test", &debug_keypoints).unwrap();
        wait_key(0).unwrap();*/

        ProcessedImage {
            source_img: img,
            descriptors: result.descriptors,
            keypoints: result.keypoints.iter().collect(),
            small_img: to_small_image(&normalized_img_mat),
            img: normalized_img_mat,
        }
    }
}

struct OpenCVVideoMatcher<I: Send> {
    images: Vec<ProcessedImage<I>>,
    shared_flanns: Arc<ThreadLocal<RefCell<FlannMatcher>>>,
}

impl<I: MatchableImage + Send + Copy + Eq, P: ProgressReporter + Copy + Send + Sync>
    VideoMatcher<I, P> for OpenCVVideoMatcher<I>
{
    fn match_images_with_video(&self, video_path: &Path, progress_reporter: P) -> Vec<Matching<I>> {
        let results = Arc::new(Mutex::new(Vec::<Matching<I>>::new()));

        rayon::scope_fifo(|s| {
            let vid = VideoCaptureIter::open(video_path, Duration::from_secs(5));
            let total_time = vid.total_time();
            let total_frames = vid.total_frames();
            let video_frames = FilterIter::new(vid);

            // Add a matching to indicate the last frame.
            let results = results.clone();
            let mut list = results.lock().unwrap();
            list.push(Matching {
                image: None,
                video_frame_idx: total_frames as usize,
                video_time: total_time,
            });

            for (frame, frame_time, frame_idx) in video_frames {
                let results = results.clone();
                s.spawn_fifo(move |_s| {
                    let matching = self.match_images_with_frame(
                        frame,
                        frame_time,
                        frame_idx,
                        progress_reporter,
                    );

                    let mut list = results.lock().unwrap();
                    list.push(matching);
                    let progress = (frame_idx as f64) / total_frames;
                    progress_reporter.report(progress as f32);
                });
            }
        });

        let mut mappings = results.lock().unwrap().clone();
        mappings.sort_by_key(|m| m.video_time);
        let mut cleaned_mappings = Vec::new();
        let mut last_mapping: Option<Matching<I>> = None;

        for mapping in mappings {
            if let Some(last_mapping) = &last_mapping {
                if last_mapping.image == mapping.image {
                    continue;
                }
            }
            last_mapping = Some(mapping.clone());
            cleaned_mappings.push(mapping);
        }

        cleaned_mappings
    }
}

impl<I: MatchableImage + Send + Copy> OpenCVVideoMatcher<I> {
    fn match_images_with_frame<P: ProgressReporter + Copy>(
        &self,
        frame: Mat,
        frame_time: Duration,
        frame_idx: usize,
        _progress_reporter: P,
    ) -> Matching<I> {
        let mut flann = self
            .shared_flanns
            .get_or(|| {
                RefCell::new(FlannMatcher::new(
                    self.images.iter().map(|f| f.descriptors.clone()),
                ))
            })
            .borrow_mut();

        let frame_info =
            FEATURE_EXTRACTOR.with(|e| e.borrow_mut().find_keypoints_and_descriptors(&frame));
        let matches = flann.knn_match(&frame_info.descriptors, 30);

        let mut best_matches_by_slide_idx = HashMap::<usize, Vec<_>>::new();

        for matched_descriptors in matches.into_iter() {
            let best = matched_descriptors[0].clone();
            for dmatch in matched_descriptors.into_iter() {
                // Add a match for all descriptors the query descriptor has a good match with.
                assert!(best.query_idx == dmatch.query_idx);
                if dmatch.distance < best.distance * 1.05 {
                    best_matches_by_slide_idx
                        .entry(dmatch.source)
                        .or_default()
                        .push(dmatch);
                }
            }
        }

        let mut best_matches = best_matches_by_slide_idx
            .into_iter()
            .map(|(idx, x)| (&self.images[idx], x))
            .collect::<Vec<_>>();

        // Process slides with many matches first
        best_matches.sort_by_key(|(_, m)| -(m.len() as isize));

        let mut rated_best_matches: Vec<_> = best_matches
            .into_iter()
            // Only consider the best 40 slides
            .take(40)
            .map(|(slide_info, matches)| {
                let result = Transformation2D::estimate_affine(matches.iter().map(|m| {
                    (
                        slide_info.keypoints.get(m.train_idx as usize).unwrap().pt,
                        frame_info.keypoints.get(m.query_idx as usize).unwrap().pt,
                    )
                }));
                let inlier_matches: Vec<_> = matches
                    .into_iter()
                    .zip(result.inlier_flags())
                    .filter(|&(_, is_inlier)| is_inlier)
                    .map(|(m, _)| m)
                    .collect();

                let rating = inlier_matches.len() as f64;
                (slide_info, inlier_matches, rating, result.transformation)
            })
            .collect();

        /*
        let mut debug_keypoints = Mat::default().unwrap();
        draw_keypoints(
            &frame,
            &frame_info.keypoints,
            &mut debug_keypoints,
            Scalar::new(255.0, 0.0, 0.0, 0.0),
            DrawMatchesFlags::DEFAULT,
        )
        .unwrap();
        imshow(&"test", &debug_keypoints).unwrap();
        wait_key(0).unwrap();
        */

        rated_best_matches.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
        rated_best_matches.truncate(10);
        let best_rating = rated_best_matches.first().map_or(0.0, |v| v.2);
        // Keep all matches that have a competitive rating.
        rated_best_matches.retain(|v| v.2 > 50.0 && v.2 / best_rating > 0.2);

        let mut rated_best_matches = rated_best_matches
            .into_iter()
            .map(|(slide_info, matches, _rating, transformation)| {
                let mut frame_proj = Mat::default().unwrap();
                warp_affine(
                    &frame,
                    &mut frame_proj,
                    &transformation.mat,
                    slide_info.img.size().unwrap(),
                    WARP_INVERSE_MAP,
                    0,
                    Scalar::new(0.0, 0.0, 0.0, 0.0),
                )
                .unwrap();

                let frame_proj2 = to_small_image(&frame_proj);
                let similarity = get_similarity(&frame_proj2, &slide_info.small_img);
                /*
                println!("similarity: {}, rating: {}", similarity, rating);
                imshow(&"test", &frame_proj2).unwrap();
                imshow(&"test2", &slide_info.small_img).unwrap();
                wait_key(0).unwrap();
                */

                /*
                let mut images = Vector::<Mat>::default();
                images.push(frame_proj2);
                images.push(slide_info.small_img.clone());
                let mut out = Mat::default().unwrap();
                hconcat(&images, &mut out).unwrap();*/

                (slide_info, matches, similarity, transformation)
            })
            .collect::<Vec<_>>();

        rated_best_matches.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
        println!(
            "best sim: {:?} best rating: {}",
            rated_best_matches.iter().next().map(|v| v.2),
            best_rating
        );

        // At least a similarity of 0.5 is required
        rated_best_matches.retain(|v| v.2 > 0.5);

        let first = rated_best_matches.into_iter().next();

        Matching {
            video_frame_idx: frame_idx,
            video_time: frame_time,
            image: first.map(|v| v.0.source_img),
        }

        /*
        if let Some((slide_info, _matches, _rating, _transformation, out)) = first {
            let mut list = results.lock().unwrap();
            /*std::fs::create_dir_all(&out_path).unwrap();
            imwrite(
                &out_path
                    .join(&format!(
                        "{}-{}.png",
                        frame_timestamp.as_secs(),
                        slide_info.page
                    ))
                    .to_string_lossy(),
                &out,
                &Vector::new(),
            )
            .unwrap();
            */
            list.borrow_mut().push(Mapping {
                offset_ms: frame_timestamp.as_millis() as usize,
                slide_idx: slide_info.page as usize,
            });
        }*/
    }
}

/*
let mut frame_clone = Mat::default().unwrap();
warp_affine(
    &slide_info.img,
    &mut frame_clone,
    &transformation.mat,
    frame.size().unwrap(),
    0,
    0,
    Scalar::new(0.0, 0.0, 0.0, 0.0),
)
.unwrap();

let mut out = Mat::default().unwrap();
cvt_color(&frame_clone, &mut out, COLOR_BGRA2BGR, 0).unwrap();

let mut target = Mat::default().unwrap();
add_weighted(&out, 0.5, &frame, 0.5, 0.0, &mut target, -1).unwrap();
*/

/*let v = matches
    .iter()
    .map(|m| iter::once(m.clone()).collect::<Vector<_>>())
    .collect::<Vector<_>>();

let mut out_img = Mat::default().unwrap();
draw_matches_knn(
    &frame,
    &frame_info.keypoints,
    &slide_info.img,
    &slide_info.keypoints,
    &v,
    &mut out_img,
    Scalar::new(255.0, 0.0, 0.0, 0.0),
    Scalar::new(0.0, 255.0, 0.0, 0.0),
    &Vector::default(),
    DrawMatchesFlags::NOT_DRAW_SINGLE_POINTS,
)
.unwrap();

*/
