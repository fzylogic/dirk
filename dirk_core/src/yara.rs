use std::default::Default;
use std::path::Path;

use base64::{engine::general_purpose, Engine as _};

use crate::models::yara::*;

pub fn analyze_file_data(
    file_data: &str,
    filename: &Path,
    rules: &yara::Rules,
) -> Result<ScanResult, Box<dyn std::error::Error>> {
    let decoded = &general_purpose::STANDARD.decode(file_data).unwrap();
    let result = rules.scan_mem(decoded, 90)?;
    if result.is_empty() {
        Ok(ScanResult {
            filename: filename.to_owned(),
            status: ResultStatus::OK,
            ..Default::default()
        })
    } else {
        Ok(ScanResult {
            filename: filename.to_owned(),
            signature: result
                .into_iter()
                .map(|r| r.identifier.to_string())
                .collect(),
            status: ResultStatus::Bad,
        })
    }
}
