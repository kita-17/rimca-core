use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("zip error: {0}")]
    ZipError(#[from] zip::result::ZipError),
    #[error("reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),
}