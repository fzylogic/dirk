use crate::entities::sea_orm_active_enums::FileStatus;
use crate::models::hank::Signature;
use crate::phpxdebug::Tests;
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
    DynamicRule,
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
            DirkReason::DynamicRule => write!(f, "Dynamic Analysis rule(s) triggered"),
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
            ScanType::Dynamic => format!("{}{}", urlbase, "scanner/dynamic"),
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
    pub dynamic_results: Option<Vec<Tests>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ScanBulkResult {
    pub id: Uuid,
    pub results: Vec<ScanResult>,
}

impl ScanBulkResult {
    pub fn print_results(&self, verbose: bool) {
        let mut result_count: usize = 0;
        let mut bad_count: usize = 0;
        result_count += self.results.len();
        for result in self.results.iter() {
            let filename_tag = match result.file_names.len() {
                0 => continue,
                1 => format!("{}", &result.file_names[0].display()),
                2.. => format!(
                    "{} (and {} other names)",
                    &result.file_names[0].display(),
                    result.file_names.len() - 1
                ),
                _ => continue,
            };
            match result.result {
                DirkResultClass::OK => {
                    if verbose {
                        println!("{} passed", filename_tag)
                    }
                }
                DirkResultClass::Inconclusive => {
                    println!("{} was inconclusive", filename_tag)
                }
                DirkResultClass::Bad => {
                    match result.reason {
                        DirkReason::DynamicRule => println!(
                            "{} is BAD: {:#?}",
                            filename_tag,
                            result.dynamic_results.as_ref().unwrap()
                        ),
                        _ => println!("{} is BAD: {}", filename_tag, result.reason),
                    }
                    bad_count += 1;
                }
            }
        }
        println!(
            "Summary: Out of {} files checked, {} were bad",
            result_count, bad_count
        );
    }
}

//#[derive(Clone)]
pub struct DirkState {
    pub sigs: Vec<Signature>,
    pub db: DatabaseConnection,
}
