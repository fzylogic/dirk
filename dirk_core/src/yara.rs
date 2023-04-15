use std::default::Default;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use crate::errors::*;

use serde_json;

use crate::models::yara::*;

// TODO have this take raw data as input and write a wrapper to convert the file contents
// This should help testing later
pub fn build_sigs_from_file(filename: PathBuf) -> Result<Vec<Signature>, DirkError> {
    let file = File::open(filename)?;
    let mut results = Vec::new();
    let mut buf = String::new();
    let mut reader = BufReader::new(file);
    loop {
        let len = reader.read_line(&mut buf)?;
        if len == 0 {
            break;
        }
        let sig: Signature =
            serde_json::from_str(&buf).expect("Unable to parse line into a Signature");
        results.push(sig);
        buf.clear();
    }
    Ok(results)
}

// pub fn analyze_file(filename: &Path, sigs: &Vec<Signature>) -> Result<ScanResult, std::io::Error> {
//     let file_data = read_to_string(filename)?;
//     analyze_file_data(&file_data, filename, sigs)
// }

pub fn analyze_file_data(
    _file_data: &str,
    filename: &Path,
    rules: &yara::Rules,
) -> Result<ScanResult, std::io::Error> {
    let _scanner = rules.scanner().unwrap();

    Ok(ScanResult {
        filename: filename.to_owned(),
        status: ResultStatus::OK,
        ..Default::default()
    })
}
