use std::default::Default;
use std::fs::{read_to_string, File};
use std::io::prelude::*;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use crate::errors::*;
use base64::{engine::general_purpose, Engine as _};
use serde_json;

use crate::models::hank::*;

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
//TODO This should be a Signature method
fn decode_sig_to_pattern(sig: &Signature) -> String {
    if sig.signature.contains('\n') {
        let mut temp = String::new();
        for part in sig.signature.split('\n') {
            let decoded_part = general_purpose::STANDARD
                .decode(part)
                .expect("Unable to decode signature");
            let decoded_sig = std::str::from_utf8(&decoded_part).unwrap();
            if temp.is_empty() {
                temp = decoded_sig.to_string();
            } else {
                temp = format!("{}\n{}", &temp, &decoded_sig);
            }
        }
        temp
    } else {
        return std::str::from_utf8(
            &general_purpose::STANDARD
                .decode(&sig.signature)
                .expect("Unable to decode signature"),
        )
        .unwrap()
        .to_string();
    }
}

pub fn analyze_file(filename: &Path, sigs: &Vec<Signature>) -> Result<ScanResult, std::io::Error> {
    let file_data = read_to_string(filename)?;
    analyze_file_data(&file_data, filename, sigs)
}

pub fn analyze_file_data(
    file_data: &str,
    filename: &Path,
    sigs: &Vec<Signature>,
) -> Result<ScanResult, std::io::Error> {
    for sig in sigs {
        let pattern = decode_sig_to_pattern(sig);
        //println!("Testing pattern ({pattern})");
        if file_data.contains(&pattern) {
            return Ok(ScanResult {
                filename: filename.to_owned(),
                status: ResultStatus::Bad,
                signature: Some(sig.to_owned()),
            });
        }
    }
    Ok(ScanResult {
        filename: filename.to_owned(),
        status: ResultStatus::OK,
        ..Default::default()
    })
}
