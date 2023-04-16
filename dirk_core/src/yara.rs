use std::default::Default;
use std::path::{Path};
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
    println!("Analyzing");
    let result = rules.scan_mem(file_data.as_bytes(), 90)?;
    if result.is_empty() {
        Ok(ScanResult {
            filename: filename.to_owned(),
            status: ResultStatus::OK,
            ..Default::default()
        })
    } else {
        Ok(ScanResult {
            filename: filename.to_owned(),
            signature: Some(result.into_iter().map(|r|r.identifier.to_string()).collect()),
            status: ResultStatus::Bad,
        })
    }

}
