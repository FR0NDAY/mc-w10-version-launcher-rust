use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("xml parse error: {0}")]
    Xml(#[from] roxmltree::Error),
    #[error("time format error: {0}")]
    TimeFormat(#[from] time::error::Format),
    #[error("invalid response: {0}")]
    InvalidResponse(String),
    #[error("download url not found in response")]
    DownloadUrlNotFound,
    #[error("no download urls provided")]
    NoDownloadUrls,
}
