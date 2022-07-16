#[derive(Debug)]
pub enum Errors {
    InvalidFont,
    InvalidImageType,
    InvalidResizeFilter,
    InputImageAlreadyUsed,
    IOError(std::io::Error),
    ImageError(image::ImageError),
    #[cfg(feature = "base64")]
    Base64DecodeError(base64::DecodeError),
    #[cfg(feature = "reqwest")]
    ReqwestError(reqwest::Error),
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

#[cfg(feature = "base64")]
impl From<base64::DecodeError> for Errors {
    fn from(error: base64::DecodeError) -> Self {
        Self::Base64DecodeError(error)
    }
}

#[cfg(feature = "reqwest")]
impl From<reqwest::Error> for Errors {
    fn from(error: reqwest::Error) -> Self {
        Self::ReqwestError(error)
    }
}
