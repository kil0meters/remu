#[derive(thiserror::Error, Debug)]
pub enum RVError {
    #[error("segmentation fault")]
    SegmentationFault,

    #[error("the requested function label does not exist")]
    InvalidLabel,

    #[error("The requested file type is not valid")]
    InvalidFileType,
}
