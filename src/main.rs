mod feature_extractor;
mod flann;
mod image_utils;
mod utils;
mod video_capture;

use feature_extractor::FeatureExtractor;
use flann::FlannMatcher;
use image_utils::Transformation2D;

use opencv::imgcodecs::*;
use opencv::{
    calib3d::{estimate_affine_2d, RANSAC},
    core::{count_non_zero, no_array, DMatch, KeyPoint, Point2f, Ptr, Scalar, Size, Vector},
    features2d::{draw_keypoints, draw_matches_knn, DrawMatchesFlags},
    flann::{IndexParams, LshIndexParams, SearchParams, FLANN_INDEX_LSH},
    highgui::{imshow, wait_key},
    imgproc::{resize, INTER_AREA},
    prelude::*,
    types::VectorOfMat,
    videoio::{VideoCapture, CAP_DSHOW, CAP_PROP_FPS, CAP_PROP_FRAME_COUNT, CAP_PROP_POS_FRAMES},
};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::{
    borrow::{Borrow, BorrowMut},
    cell::{Cell, RefCell},
    collections::HashMap,
    iter,
    path::{Path, PathBuf},
    process::Command,
    rc::Rc,
    time::Instant,
};
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
}

fn test() -> opencv::Result<()> {
    let slides = pdf_to_images(&PathBuf::from(
        &"S:\\dev\\2021\\slide-synchronizer\\data\\slides2.pdf",
    ));

    let slides_with_features: Vec<_> = slides
        .par_iter()
        .map(|i| {
            let slide = imread(&i.to_string_lossy(), 0).unwrap();
            let result =
                FEATURE_EXTRACTOR.with(|e| e.borrow_mut().find_keypoints_and_descriptors(&slide));
            Slide {
                descriptors: result.descriptors,
                keypoints: result.keypoints,
                page: 0,
                img: slide,
            }
        })
        .collect();

    let mut flann = FlannMatcher::default();
    flann.add_descriptors(slides_with_features.iter().map(|f| f.descriptors.clone()));
    flann.train();

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

        let mut best_matches_by_slide_idx = HashMap::<i32, Vec<DMatch>>::new();

        for matched_descriptors in matches {
            let best = &matched_descriptors.get(0).unwrap();

            for dmatch in matched_descriptors {
                assert!(best.query_idx == dmatch.query_idx);
                if dmatch.distance < best.distance * 1.05 {
                    best_matches_by_slide_idx
                        .entry(dmatch.img_idx)
                        .or_default()
                        .push(dmatch);
                }
            }
        }

        let mut best_matches = best_matches_by_slide_idx.into_iter().collect::<Vec<_>>();
        best_matches.sort_by_key(|(_, m)| -(m.len() as isize));

        let mut rated_best_matches: Vec<_> = best_matches
            .into_iter()
            .take(40)
            .map(|(slide_idx, matches)| {
                let slide_info = &slides_with_features[slide_idx as usize];

                //get_rating_of_affine_transform

                let result = Transformation2D::estimate(matches.iter().map(|m| {
                    (
                        slide_info.keypoints.get(m.train_idx as usize).unwrap().pt,
                        frame_info.keypoints.get(m.query_idx as usize).unwrap().pt,
                    )
                }));

                let inlier_matches: Vec<DMatch> = matches
                    .into_iter()
                    .zip(result.inlier_flags())
                    .filter_map(|(m, is_inlier)| if is_inlier { Some(m) } else { None })
                    .collect();

                //let affine_rating = result.rating();
                //println!("Rating of slide {}: {}", slide_idx, affine_rating);

                let rating = inlier_matches.len() as f64;

                (slide_idx, inlier_matches, rating)
            })
            .collect();

        rated_best_matches.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

        let first = rated_best_matches.into_iter().next();
        if first.is_none() {
            continue;
        }

        let (slide_idx, matches, _rating) = first.unwrap();
        let slide_info = &slides_with_features[slide_idx as usize];

        let v = matches
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
        imshow(&"test", &out_img)?;
        wait_key(0)?;

        println!("next");
        //process_img(&frame);
    }

    return Ok(());
}
