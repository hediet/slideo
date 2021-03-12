use opencv::{
    core::{no_array, KeyPoint, Mat, Ptr, Vector},
    features2d::{self, ORB_ScoreType},
    prelude::{Feature2DTrait, ORB},
};

pub struct FeatureExtractor {
    ptr: Ptr<dyn features2d::ORB>,
}

#[derive(Debug)]
pub struct KeypointsWithDescriptors {
    pub keypoints: Vector<KeyPoint>,
    pub descriptors: Mat,
}

impl FeatureExtractor {
    pub fn default() -> FeatureExtractor {
        let orb = ORB::create(
            /* nfeatures: */ 2000,
            /* scale_factor: */ 1.2,
            /* nlevels: */ 8,
            /* edge_threshold: */ 62,
            /* first_level: */ 0,
            /* wta_k: */ 2,
            /* score_type: */ ORB_ScoreType::FAST_SCORE,
            /* patch_size: */ 62,
            /* fast_threshold: */ 20,
        )
        .unwrap();

        return FeatureExtractor { ptr: orb };
    }

    pub fn find_keypoints_and_descriptors(&mut self, src: &Mat) -> KeypointsWithDescriptors {
        let mut descriptors = Mat::default().unwrap();
        let mut keypoints = Vector::new();
        self.ptr
            .detect_and_compute(
                src,
                &no_array().unwrap(),
                &mut keypoints,
                &mut descriptors,
                false,
            )
            .unwrap();

        return KeypointsWithDescriptors {
            descriptors,
            keypoints,
        };
    }
}
