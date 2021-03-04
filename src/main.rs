mod feature_extractor;
mod flann;
mod image_utils;
mod utils;
mod video_capture;

use feature_extractor::FeatureExtractor;
use flann::{FlannDictionary, FlannMatcher};
use image_utils::{get_similarity, to_small_image, Transformation2D};

use opencv::{
    core::{add_weighted, DMatch, KeyPoint, Scalar, Vector},
    features2d::{draw_matches_knn, DrawMatchesFlags},
    highgui::{imshow, wait_key},
    imgproc::{cvt_color, COLOR_BGRA2BGR, WARP_INVERSE_MAP},
    prelude::*,
};
use opencv::{imgcodecs::*, imgproc::warp_affine};
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use std::{cell::RefCell, collections::HashMap, hash::Hasher, iter, path::PathBuf, time::Instant};
use utils::pdf_to_images;
use video_capture::{FilterIter, VideoCaptureIter};

fn main() {
    let now = Instant::now();
    test().unwrap();
    println!("{:?}", now.elapsed());
}

thread_local! {
    pub static FEATURE_EXTRACTOR: RefCell<FeatureExtractor> = RefCell::new(FeatureExtractor::default());
}

struct Slide {
    pub keypoints: Vector<KeyPoint>,
    pub descriptors: Mat,
    pub page: u32,
    pub img: Mat,
    pub small_img: Mat,
}

impl PartialEq for &Slide {
    fn eq(&self, other: &Self) -> bool {
        self.page == other.page
    }
}

impl Eq for &Slide {}

impl std::hash::Hash for &Slide {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u32(self.page);
    }
}

fn test() -> opencv::Result<()> {
    let slides = pdf_to_images(&PathBuf::from(
        &"S:\\dev\\2021\\slide-synchronizer\\data\\slides2.pdf",
    ));

    let slides_with_features: Vec<_> = slides
        .par_iter()
        .enumerate()
        .map(|(idx, i)| {
            let slide = imread(&i.to_string_lossy(), 0).unwrap();
            let mut slide2 = Mat::default().unwrap();
            cvt_color(&slide, &mut slide2, COLOR_BGRA2BGR, 0).unwrap();

            let result =
                FEATURE_EXTRACTOR.with(|e| e.borrow_mut().find_keypoints_and_descriptors(&slide2));
            Slide {
                descriptors: result.descriptors,
                keypoints: result.keypoints,
                page: idx as u32,
                small_img: to_small_image(&slide2),
                img: slide2,
            }
        })
        .collect();

    let mut flann = FlannDictionary::new(
        slides_with_features
            .iter()
            .map(|f| (f, f.descriptors.clone())),
    );

    let vid = VideoCaptureIter::open(
        &"S:\\uni\\Entscheidungsverfahren\\Vorlesung vom 10.11.2020.mp4".into(),
        5.0,
    );
    let total_frames = vid.total_frames();
    let video_frames = FilterIter::new(vid);

    for (frame_idx, frame, _scaled_frame) in video_frames {
        println!(
            "Processing next frame ({:?}% done)",
            (frame_idx / total_frames) * 100.0
        );
        let frame_info =
            FEATURE_EXTRACTOR.with(|e| e.borrow_mut().find_keypoints_and_descriptors(&frame));
        println!("Find matches...");
        let matches = flann.knn_match(&frame_info.descriptors, 30);

        let mut best_matches_by_slide_idx = HashMap::<&Slide, Vec<_>>::new();
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

        let mut best_matches = best_matches_by_slide_idx.into_iter().collect::<Vec<_>>();
        // Process slides with many matches first
        best_matches.sort_by_key(|(_, m)| -(m.len() as isize));

        let mut rated_best_matches: Vec<_> = best_matches
            .into_iter()
            .take(40) // Only consider the best 40 slides
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

        rated_best_matches.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
        rated_best_matches.truncate(10);

        let mut rated_best_matches = rated_best_matches
            .into_iter()
            .map(|(slide_info, matches, rating, transformation)| {
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

                let similarity =
                    get_similarity(&to_small_image(&frame_proj), &slide_info.small_img);
                //println!("similarity: {}", similarity);

                /*
                imshow(&"test", &frame_proj2).unwrap();
                imshow(&"test2", &slide_info.small_img).unwrap();
                wait_key(0).unwrap();
                */
                (slide_info, matches, similarity, transformation)
            })
            .collect::<Vec<_>>();

        rated_best_matches.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

        let first = rated_best_matches.into_iter().next();
        if first.is_none() {
            continue;
        }

        let (slide_info, matches, _rating, transformation) = first.unwrap();
    }

    return Ok(());
}
