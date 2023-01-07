use reqwest;

#[derive(Debug)]
pub enum ScanError {
    ReqwestError(reqwest::Error),
    IOError(std::io::Error),
}

impl From<std::io::Error> for ScanError {
    fn from(error: std::io::Error) -> Self {
        ScanError::IOError(error)
    }
}

impl From<reqwest::Error> for ScanError {
    fn from(error: reqwest::Error) -> Self {
        ScanError::ReqwestError(error)
    }
}
