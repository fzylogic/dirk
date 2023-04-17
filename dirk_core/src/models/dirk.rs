use std::fmt;
use std::path::PathBuf;

use axum::http::Uri;
use clap::ValueEnum;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use yara;
use crate::entities::file_rule_match;

use crate::entities::sea_orm_active_enums::FileStatus;
use crate::phpxdebug::Tests;

/// The Type of result we've received about a file
#[derive(Copy, Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub enum DirkResultClass {
    Bad,
    Inconclusive,
    #[default]
    OK,
}

/// The reasoning behind the result we received
#[derive(Copy, Clone, Debug, Default, Deserialize, Serialize)]
pub enum DirkReason {
    Cached,
    DynamicRule,
    InternalError,
    YaraRule,
    #[default]
    None,
}

/// Request to update a file record
#[derive(Debug, Deserialize, Serialize)]
pub struct FileUpdateRequest {
    pub checksum: String,
    pub file_status: FileStatus,
    pub rule_matches: Vec<String>,
}

impl fmt::Display for DirkReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DirkReason::Cached => write!(f, "Cached SHA Checksum"),
            DirkReason::InternalError => write!(f, "Internal Error encountered"),
            DirkReason::None => write!(f, "No reason; something must have gone wrong"),
            DirkReason::YaraRule => write!(f, "Yara rule was triggered"),
            DirkReason::DynamicRule => write!(f, "Dynamic Analysis rule(s) triggered"),
        }
    }
}

/// The types of scan requests currently supported
#[derive(Clone, Debug, Default, ValueEnum, Deserialize, Serialize)]
pub enum ScanType {
    Dynamic,
    FindUnknown,
    Full,
    #[default]
    Quick,
}

/// The typed of submission requests currently supported
#[derive(Clone, Debug, Default, ValueEnum, Deserialize, Serialize)]
pub enum SubmissionType {
    List,
    #[default]
    Update,
}

pub trait DirkUrl {
    fn url(&self, urlbase: Uri) -> String;
}

impl DirkUrl for ScanType {
    fn url(&self, urlbase: Uri) -> String {
        match self {
            ScanType::Dynamic => format!("{}{}", urlbase, "scanner/dynamic"),
            ScanType::Full => format!("{}{}", urlbase, "scanner/full"),
            ScanType::Quick => format!("{}{}", urlbase, "scanner/quick"),
            _ => "".to_string(),
        }
    }
}

impl DirkUrl for SubmissionType {
    fn url(&self, urlbase: Uri) -> String {
        match self {
            SubmissionType::Update => format!("{}{}", urlbase, "files"),
            SubmissionType::List => format!("{}{}", urlbase, "files"),
        }
    }
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

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ScanRequest {
    pub sha1sum: String,
    pub kind: ScanType,
    pub file_name: PathBuf,
    pub file_contents: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ScanBulkRequest {
    pub requests: Vec<ScanRequest>,
    pub skip_cache: bool,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ScanResult {
    pub file_names: Vec<PathBuf>,
    pub sha1sum: String,
    pub result: DirkResultClass,
    pub reason: DirkReason,
    pub cache_detail: Option<FileStatus>,
    pub signature: Vec<String>,
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
                        _ => println!(
                            "{} is BAD: {:?}",
                            filename_tag,
                            result.signature.clone()
                        ),
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

/// Internal API state
pub struct DirkState {
    pub rules: yara::Rules,
    pub db: DatabaseConnection,
}
