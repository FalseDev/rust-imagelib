#[derive(Debug)]
pub enum Errors {
    IOError(std::io::Error),
    ImageError(image::ImageError),
    InputImageAlreadyUsed,
}

impl From<image::ImageError> for Errors {
    fn from(error: image::ImageError) -> Self {
        Self::ImageError(error)
    }
}

impl From<std::io::Error> for Errors {
    fn from(error: std::io::Error) -> Self {
        Self::IOError(error)
    }
}
