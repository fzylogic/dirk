use axum::http::uri::InvalidUri;
use reqwest;

#[derive(Debug)]
pub enum DirkError {
    ArgumentError,
    DbError(sea_orm::DbErr),
    InvalidUri(InvalidUri),
    IOError(std::io::Error),
    ReqwestError(reqwest::Error),
}

impl From<sea_orm::DbErr> for DirkError {
    fn from(error: sea_orm::DbErr) -> Self {
        DirkError::DbError(error)
    }
}

impl From<InvalidUri> for DirkError {
    fn from(error: InvalidUri) -> Self {
        DirkError::InvalidUri(error)
    }
}

impl From<std::io::Error> for DirkError {
    fn from(error: std::io::Error) -> Self {
        DirkError::IOError(error)
    }
}

impl From<reqwest::Error> for DirkError {
    fn from(error: reqwest::Error) -> Self {
        DirkError::ReqwestError(error)
    }
}
