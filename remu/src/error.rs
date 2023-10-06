#[derive(thiserror::Error, Debug)]
pub enum RVError {
    #[error("segmentation fault")]
    SegmentationFault,
}
