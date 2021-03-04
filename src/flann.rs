
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
use opencv::{features2d::FlannBasedMatcher};
use rayon::iter::{ParallelIterator};


pub struct FlannMatcher {
    matcher: FlannBasedMatcher,
}

impl FlannMatcher {
    pub fn default() -> Self {
        let mut index_params = Ptr::new(IndexParams::default().unwrap());
        index_params.set_int(&"table_number", 6).unwrap();
        index_params.set_int(&"key_size", 12).unwrap();
        index_params.set_int(&"multi_probe_level", 1).unwrap();
        index_params.set_algorithm(FLANN_INDEX_LSH).unwrap();

        let search_params = Ptr::new(SearchParams::new(32, 0.0, true).unwrap());

        let matcher = FlannBasedMatcher::new(&index_params, &search_params).unwrap();

        return FlannMatcher { matcher };
    }

    pub fn add_descriptors<'a, I>(self: &mut Self, descriptors: I)
    where
        I: Iterator<Item = Mat>,
    {
        let vec: VectorOfMat = descriptors.collect();
        opencv::prelude::FlannBasedMatcherTrait::add(&mut self.matcher, &vec).unwrap();
    }

    pub fn knn_match(self: &mut Self, descriptors: &Mat, k: i32) -> Vector<Vector<DMatch>> {
        let mut matches = Vector::new();
        let masks = &no_array().unwrap();
        self.matcher
            .knn_match(descriptors, &mut matches, k, masks, false)
            .unwrap();
        return matches;
    }

    pub fn train(self: &mut Self) {
        opencv::prelude::FlannBasedMatcherTrait::train(&mut self.matcher).unwrap();
    }
}
