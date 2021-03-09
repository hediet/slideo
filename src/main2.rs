mod feature_extractor;
mod flann;
mod image_utils;
mod utils;
mod video_capture;

use feature_extractor::FeatureExtractor;
use flann::FlannDictionary;
use image_utils::{get_similarity, to_small_image, Transformation2D};

use opencv::{
    core::{hconcat, KeyPoint, Scalar, Vector},
    highgui::{imshow, wait_key},
    imgproc::{cvt_color, COLOR_BGRA2BGR, WARP_INVERSE_MAP},
    prelude::*,
};
use opencv::{imgcodecs::*, imgproc::warp_affine};
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use std::{
    borrow::BorrowMut,
    cell::RefCell,
    collections::HashMap,
    fmt::Debug,
    hash::Hasher,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use utils::{get_temp_path_key, pdf_to_images};
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
    pub keypoints: Vec<KeyPoint>,
    pub descriptors: Mat,
    pub page: u32,
    pub img: Mat,
    pub small_img: Mat,
}

unsafe impl Send for Slide {}
unsafe impl Sync for Slide {}

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

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Output {
    mappings: Vec<Mapping>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Mapping {
    offset_ms: usize,
    slide_idx: usize,
}

fn test() -> opencv::Result<()> {
    let results = Arc::new(Mutex::new(Vec::<Mapping>::new()));
    let out_path = get_temp_path_key(&"matchings", &format!("{:?}", std::time::SystemTime::now()));
    rayon::scope(|s| {
        let slides = pdf_to_images(&PathBuf::from(
            &"S:\\uni\\Formale Systeme 2 Theorie\\pdfs\\all.pdf",
            //&"S:\\dev\\2021\\slide-synchronizer\\data\\slides2.pdf",
        ));

        let slides_with_features: Box<Vec<Slide>> = Box::new(
            slides
                .par_iter()
                .enumerate()
                .map(|(idx, i)| {
                    let slide = imread(&i.to_string_lossy(), 0).unwrap();
                    let mut slide2 = Mat::default().unwrap();
                    cvt_color(&slide, &mut slide2, COLOR_BGRA2BGR, 0).unwrap();

                    let result = FEATURE_EXTRACTOR
                        .with(|e| e.borrow_mut().find_keypoints_and_descriptors(&slide2));
                    Slide {
                        descriptors: result.descriptors,
                        keypoints: result.keypoints.iter().collect(),
                        page: idx as u32,
                        small_img: to_small_image(&slide2),
                        img: slide2,
                    }
                })
                .collect(),
        );
        let slides_with_features: &Vec<Slide> = Box::leak::<'static>(slides_with_features);

        let vid = VideoCaptureIter::open(
            //&"S:\\uni\\Entscheidungsverfahren\\Vorlesung vom 10.11.2020.mp4".into(),
            &"S:\\uni\\Formale Systeme 2 Theorie\\videos\\SocialChoice2.mp4".into(),
            Duration::from_secs(5),
        );

        let total_time = vid.total_time();
        let video_frames = FilterIter::new(vid);

        thread_local!(static SHARED_FLANN: RefCell<Option<FlannDictionary::<&'static Slide>>> = RefCell::new(None));

        for (frame_timestamp, frame, _scaled_frame) in video_frames {
            let results = results.clone();
            let out_path = out_path.clone();
            s.spawn(move |s| {
                SHARED_FLANN.with(|flann: &RefCell<Option<FlannDictionary<&'static Slide>>>| {
                    println!(
                        "Processing next frame ({}, {:.1}%)",
                        fmt_duration(frame_timestamp),
                        (frame_timestamp.as_secs_f64() / total_time.as_secs_f64()) * 100.0
                    );

                    if flann.borrow().is_none() {
                        flann.replace(Some(FlannDictionary::new(
                            slides_with_features
                                .iter()
                                .map(|f| (f, f.descriptors.clone())),
                        )));
                    }

                    let mut flann = flann.borrow_mut();
                    let flann = flann.as_mut().unwrap();

                    let frame_info = FEATURE_EXTRACTOR
                        .with(|e| e.borrow_mut().find_keypoints_and_descriptors(&frame));
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

                    let mut best_matches =
                        best_matches_by_slide_idx.into_iter().collect::<Vec<_>>();
                    // Process slides with many matches first
                    best_matches.sort_by_key(|(_, m)| -(m.len() as isize));

                    let mut rated_best_matches: Vec<_> = best_matches
                        .into_iter()
                        .take(40) // Only consider the best 40 slides
                        .map(|(slide_info, matches)| {
                            let result =
                                Transformation2D::estimate_affine(matches.iter().map(|m| {
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
                    let best_rating = rated_best_matches.first().map_or(0.0, |v| v.2);
                    // Keep all matches that have a competitive rating.
                    rated_best_matches.retain(|v| v.2 > 50.0 && v.2 / best_rating > 0.2);

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

                            let frame_proj2 = to_small_image(&frame_proj);
                            let similarity = get_similarity(&frame_proj2, &slide_info.small_img);
                            let mut images = Vector::<Mat>::default();
                            images.push(frame_proj2);
                            images.push(slide_info.small_img.clone());
                            let mut out = Mat::default().unwrap();
                            hconcat(&images, &mut out).unwrap();

                            /*
                            println!("similarity: {}, rating: {}", similarity, rating);
                            imshow(&"test", frame_proj2).unwrap();
                            imshow(&"test2", &slide_info.small_img).unwrap();
                            wait_key(0).unwrap();
                            */

                            (slide_info, matches, similarity, transformation, out)
                        })
                        .collect::<Vec<_>>();

                    // At least a similarity of 0.5 is required
                    rated_best_matches.retain(|v| v.2 > 0.5);
                    rated_best_matches.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

                    let first = rated_best_matches.into_iter().next();

                    if let Some((slide_info, _matches, _rating, _transformation, out)) = first {
                        let mut list = results.lock().unwrap();
                        std::fs::create_dir_all(&out_path).unwrap();
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
                        list.borrow_mut().push(Mapping {
                            offset_ms: frame_timestamp.as_millis() as usize,
                            slide_idx: slide_info.page as usize,
                        });
                    }
                });
            });
        }
    });

    let mut mappings = results.lock().unwrap().clone();
    mappings.sort_by_key(|m| m.offset_ms);
    let mut cleaned_mappings = Vec::new();
    let mut last_mapping: Option<Mapping> = None;
    for mapping in mappings {
        if let Some(last_mapping) = &last_mapping {
            if last_mapping.slide_idx == mapping.slide_idx {
                continue;
            }
        }
        last_mapping = Some(mapping.clone());
        cleaned_mappings.push(mapping);
    }

    let output = Output {
        mappings: cleaned_mappings,
    };

    let serialized = serde_json::to_string(&output).unwrap();
    println!("serialized = {}", serialized);

    return Ok(());
}

fn fmt_duration(d: Duration) -> String {
    let seconds = d.as_secs() % 60;
    let minutes = (d.as_secs() / 60) % 60;
    let hours = (d.as_secs() / 60) / 60;
    return format!("{}:{}:{}", hours, minutes, seconds);
}
