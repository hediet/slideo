use opencv::features2d::FlannBasedMatcher;
use opencv::{
    core::{no_array, DMatch, Ptr, Vector},
    flann::{IndexParams, SearchParams, FLANN_INDEX_LSH},
    prelude::*,
    types::VectorOfMat,
};

struct BaseFlannMatcher {
    matcher: FlannBasedMatcher,
}

impl BaseFlannMatcher {
    pub fn default() -> Self {
        let mut index_params = Ptr::new(IndexParams::default().unwrap());
        index_params.set_int(&"table_number", 6).unwrap();
        index_params.set_int(&"key_size", 12).unwrap();
        index_params.set_int(&"multi_probe_level", 1).unwrap();
        index_params.set_algorithm(FLANN_INDEX_LSH).unwrap();

        let search_params = Ptr::new(SearchParams::new(32, 0.0, true).unwrap());

        let matcher = FlannBasedMatcher::new(&index_params, &search_params).unwrap();

        return BaseFlannMatcher { matcher };
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

#[derive(Clone, Debug, PartialEq)]
pub struct KeyedDMatch<TKey> {
    /// query descriptor index
    pub query_idx: i32,
    /// train descriptor index
    pub train_idx: i32,
    pub source: TKey,
    pub distance: f32,
}

pub struct FlannMatcher {
    matcher: BaseFlannMatcher,
}

impl FlannMatcher {
    pub fn new<'a, I>(descriptors: I) -> Self
    where
        I: Iterator<Item = Mat>,
    {
        let mut matcher = BaseFlannMatcher::default();
        matcher.add_descriptors(descriptors);
        matcher.train();

        FlannMatcher { matcher }
    }

    pub fn knn_match(self: &mut Self, descriptors: &Mat, k: i32) -> Vec<Vec<KeyedDMatch<usize>>> {
        let result = self.matcher.knn_match(descriptors, k);

        result
            .into_iter()
            .map(|v| {
                v.into_iter()
                    .map(|m| KeyedDMatch {
                        distance: m.distance,
                        query_idx: m.query_idx,
                        train_idx: m.train_idx,
                        source: m.img_idx as usize,
                    })
                    .collect()
            })
            .collect()
    }
}
