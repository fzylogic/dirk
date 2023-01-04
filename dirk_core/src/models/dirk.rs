use crate::entities::sea_orm_active_enums::FileStatus;
use crate::models::hank::Signature;
use axum::http::Uri;
use clap::ValueEnum;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use uuid::Uuid;

/// The Type of result we've received about a file
#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
pub enum DirkResultClass {
    Bad,
    Inconclusive,
    OK,
}

/// The reasoning behind the result we received
#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
pub enum DirkReason {
    Cached,
    InternalError,
    LegacyRule,
    None,
}

/// Request to update a file record
#[derive(Debug, Deserialize, Serialize)]
pub struct FileUpdateRequest {
    pub checksum: String,
    pub file_status: FileStatus,
}

impl fmt::Display for DirkReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DirkReason::Cached => write!(f, "Cached SHA256SUM"),
            DirkReason::InternalError => write!(f, "Internal Error encountered"),
            DirkReason::None => write!(f, "No reason; something must have gone wrong"),
            DirkReason::LegacyRule => write!(f, "Legacy Hank rule was triggered"),
        }
    }
}

/// The typed of scan requests currently supported
#[derive(Clone, Debug, ValueEnum, Deserialize, Serialize)]
pub enum ScanType {
    Dynamic,
    FindUnknown,
    Full,
    Quick,
}

impl ScanType {
    pub fn url(&self, urlbase: Uri) -> String {
        match self {
            ScanType::Dynamic => format!("{}{}", urlbase, "scanner/dynamic/single"),
            ScanType::Full => format!("{}{}", urlbase, "scanner/full"),
            ScanType::Quick => format!("{}{}", urlbase, "scanner/quick"),
            _ => todo!(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ScanRequest {
    pub sha256sum: String,
    pub kind: ScanType,
    pub file_name: PathBuf,
    pub file_contents: Option<String>,
    pub skip_cache: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ScanBulkRequest {
    pub requests: Vec<ScanRequest>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ScanResult {
    pub file_names: Vec<PathBuf>,
    pub sha256sum: String,
    pub result: DirkResultClass,
    pub reason: DirkReason,
    pub cache_detail: Option<FileStatus>,
    pub signature: Option<Signature>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ScanBulkResult {
    pub id: Uuid,
    pub results: Vec<ScanResult>,
}

//#[derive(Clone)]
pub struct DirkState {
    pub sigs: Vec<Signature>,
    pub db: DatabaseConnection,
}
