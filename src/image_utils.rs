use base64::encode;
use opencv::{
    calib3d::{estimate_affine_2d, RANSAC},
    core::{count_non_zero, no_array, norm2, Point2f, Vector, VectorRefIterator, NORM_L2},
    prelude::*,
};

pub fn get_similarity(img1: &Mat, img2: &Mat) -> f32 {
    let error_l2 = norm2(img1, img2, NORM_L2, &no_array().unwrap()).unwrap();
    let p = img1.rows() * img1.cols();
    let max_error = ((255.0 * 255.0 * 3.0) * (p as f32)).sqrt();
    return 1.0 - (error_l2 as f32) / max_error;
}

fn mat_to_base64_string(mat: Mat) -> String {
    let mut data = Vector::new();
    opencv::imgcodecs::imencode(&".png", &mat, &mut data, &Vector::new()).unwrap();
    let data = encode(data);
    return data;
}

pub struct Transformation2D {
    mat: Mat,
}

impl Transformation2D {
    pub fn new(mat: Mat) -> Transformation2D {
        Transformation2D { mat }
    }
}

pub struct EstimationResult {
    transformation: Transformation2D,
    inliers: Vector<u8>,
}

impl EstimationResult {
    pub fn rating(&self) -> f64 {
        (count_non_zero(&self.inliers).unwrap() as f64) / (self.inliers.len() as f64)
    }

    pub fn inlier_flags(&self) -> Vec<bool> {
        self.inliers.iter().map(|v| v == 1).collect()
    }
}

impl Transformation2D {
    pub fn estimate<I>(points: I) -> EstimationResult
    where
        I: Iterator<Item = (Point2f, Point2f)>,
    {
        let (from, to): (Vector<Point2f>, Vector<Point2f>) = points.unzip();

        let mut inliers = Vector::<u8>::default();

        let mat =
            estimate_affine_2d(&from, &to, &mut inliers, RANSAC, 3.0, 2000, 0.99, 10).unwrap();
        assert!(from.len() == inliers.len());

        EstimationResult {
            transformation: Transformation2D::new(mat),
            inliers,
        }
    }
}
