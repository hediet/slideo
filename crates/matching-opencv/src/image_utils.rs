use opencv::{
    calib3d::{estimate_affine_partial_2d, RANSAC},
    core::{no_array, norm2, Point2f, Size, Vector, NORM_L2},
    imgproc::{resize, INTER_AREA},
    prelude::*,
};

pub fn to_small_image(mat: &Mat) -> Mat {
    let mut scaled_mat = Mat::default().unwrap();
    let size = mat.size().unwrap();
    let max_area = 300 * 400;
    let factor = ((max_area as f32) / (size.area() as f32)).sqrt();
    let new_size = Size::new(
        ((size.width as f32) * factor) as i32,
        ((size.height as f32) * factor) as i32,
    );
    resize(&mat, &mut scaled_mat, new_size, 0.0, 0.0, INTER_AREA).unwrap();

    scaled_mat
}

pub fn get_similarity(img1: &Mat, img2: &Mat) -> f32 {
    let error_l2 = norm2(img1, img2, NORM_L2, &no_array().unwrap()).unwrap();
    let p = img1.rows() * img1.cols();
    let max_error = ((255.0 * 255.0 * 3.0) * (p as f32)).sqrt();
    return 1.0 - (error_l2 as f32) / max_error;
}

pub struct Transformation2D {
    pub mat: Mat,
}

impl Transformation2D {
    pub fn new(mat: Mat) -> Transformation2D {
        Transformation2D { mat }
    }
}

pub struct EstimationResult {
    pub transformation: Transformation2D,
    inliers: Vector<u8>,
}

impl EstimationResult {
    pub fn inlier_flags(&self) -> Vec<bool> {
        self.inliers.iter().map(|v| v == 1).collect()
    }
}

impl Transformation2D {
    pub fn estimate_affine<I>(points: I) -> EstimationResult
    where
        I: Iterator<Item = (Point2f, Point2f)>,
    {
        let (from, to): (Vector<Point2f>, Vector<Point2f>) = points.unzip();

        let mut inliers = Vector::<u8>::default();
        let mat = estimate_affine_partial_2d(&from, &to, &mut inliers, RANSAC, 3.0, 2000, 0.99, 10)
            .unwrap();
        assert!(from.len() == inliers.len());

        EstimationResult {
            transformation: Transformation2D::new(mat),
            inliers,
        }
    }
}
