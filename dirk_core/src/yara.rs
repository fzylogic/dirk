use std::default::Default;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use crate::errors::*;

use serde_json;

use crate::models::yara::*;

// pub fn analyze_file(filename: &Path, sigs: &Vec<Signature>) -> Result<ScanResult, std::io::Error> {
//     let file_data = read_to_string(filename)?;
//     analyze_file_data(&file_data, filename, sigs)
// }

pub fn analyze_file_data(
    file_data: &str,
    filename: &Path,
    rules: &yara::Rules,
) -> Result<ScanResult, Box<dyn std::error::Error>> {
    let result = rules.scan_mem(file_data.as_bytes(), 90)?;
    if result.is_empty() {
        Ok(ScanResult {
            filename: filename.to_owned(),
            status: ResultStatus::OK,
            ..Default::default()
        })
    } else {
        let first = result.first().unwrap();
        Ok(ScanResult {
            filename: filename.to_owned(),
            signature: None,
            status: ResultStatus::Bad,
        })
    }

}
