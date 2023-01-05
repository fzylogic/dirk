use clap::Parser;

use dirk_core::entities::*;
use std::collections::HashSet;
use std::fs::read_to_string;

use axum::http::Uri;
use dirk_core::models::dirk::{ScanBulkRequest, ScanBulkResult, ScanRequest, ScanResult, ScanType};
use indicatif::{ProgressBar, ProgressStyle};
use lazy_static::lazy_static;
use reqwest::StatusCode;
use std::path::PathBuf;
use std::time::Duration;
use walkdir::{DirEntry, IntoIter, WalkDir};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, value_parser)]
    recursive: bool,
    #[clap(long, value_enum, default_value_t=ScanType::Quick)]
    scan_type: ScanType,
    #[clap(short, long)]
    verbose: bool,
    #[clap(short, long, value_parser, default_value_t = String::from("http://localhost:3000"))]
    urlbase: String,
    #[clap(long, default_value_t = 500)]
    chunk_size: usize,
    #[clap(long)]
    skip_cache: bool,
    #[clap(value_parser)]
    path: PathBuf,
}

lazy_static! {
    static ref ARGS: Args = Args::parse();
}

fn prep_file_request(path: &PathBuf) -> Result<ScanRequest, std::io::Error> {
    let file_data = String::from_utf8_lossy(&std::fs::read(path)?).to_string();
    let csum = dirk_core::util::checksum(&file_data);
    let encoded = base64::encode(&file_data);
    if ARGS.verbose {
        println!("Preparing request for {}", path.display());
    }
    Ok(ScanRequest {
        sha256sum: csum,
        kind: ARGS.scan_type.clone(),
        file_contents: Some(encoded),
        file_name: path.to_owned(),
        skip_cache: ARGS.skip_cache,
    })
}

const MAX_FILESIZE: u64 = 1_000_000; // 1MB max file size to scan

fn validate_args() {
    match &ARGS.path.is_dir() {
        true => match ARGS.recursive {
            true => (),
            false => {
                panic!("Can't check a directory w/o specifying --recursive");
            }
        },
        false => (),
    }
}

fn filter_direntry(entry: &DirEntry) -> bool {
    let path = &ARGS.path;
    if entry.path().is_dir() && entry.metadata().unwrap().len() > MAX_FILESIZE {
        if ARGS.verbose {
            println!(
                "Skipping {:?} due to size: ({})",
                &entry.path().display(),
                &path.metadata().unwrap().len()
            );
        }
        return false;
    }
    true
}

async fn send_scan_req(reqs: Vec<ScanRequest>) -> Result<ScanBulkResult, reqwest::Error> {
    let urlbase: Uri = ARGS.urlbase.parse::<Uri>().unwrap();
    let url = ARGS.scan_type.url(urlbase);

    let resp = reqwest::Client::new()
        .post(url)
        .json(&ScanBulkRequest {
            requests: reqs.clone(),
        })
        .send()
        .await
        .unwrap();
    match resp.status() {
        StatusCode::OK => {}
        _ => {
            eprintln!("Received non-OK status: {}", resp.status())
        }
    }
    let resp_data = resp.json().await;
    match resp_data {
        Ok(new_post) => Ok(new_post),
        Err(e) => panic!(
            "Error encountered while reading response to {:?}: {}",
            &reqs, e
        ),
    }
}

async fn find_unknown_files() -> Result<(), reqwest::Error> {
    let urlbase: Uri = ARGS.urlbase.parse::<Uri>().unwrap();
    let resp = reqwest::Client::new()
        .get(format!("{}{}", urlbase, "files/list"))
        .send()
        .await?;
    //let mut known_files = HashSet::new();
    let file_data: Vec<files::Model> = resp.json().await?;
    //The sha256 column is a unique key in the database, so no need to check if the hash entry already exists
    let mut known_files: HashSet<String> = HashSet::new();
    file_data.into_iter().for_each(|file| {
        known_files.insert(file.sha256sum);
    });
    let walker = new_walker();
    for entry in walker.filter_entry(filter_direntry).flatten() {
        if let Ok(file_data) = read_to_string(entry.path()) {
            if !known_files.contains(dirk_core::util::checksum(&file_data).as_str()) {
                println!("{}", entry.path().display());
            }
        }
    }
    Ok(())
}

fn new_walker() -> IntoIter {
    let path = &ARGS.path;
    WalkDir::new(path).follow_links(false).into_iter()
}

fn progress_bar() -> ProgressBar {
    let bar = ProgressBar::new_spinner();
    bar.set_style(
        ProgressStyle::with_template("{spinner} [{elapsed}] {msg}")
            .unwrap()
            .tick_strings(&["🌑", "🌘", "🌗", "🌖", "🌕", "🌔", "🌓", "🌒", "🌑"]),
    );
    bar.enable_steady_tick(Duration::from_millis(200));
    bar
}

async fn process_input_quick() -> Result<(), reqwest::Error> {
    let mut reqs: Vec<ScanRequest> = Vec::new();
    let mut results: Vec<ScanResult> = Vec::new();
    let mut counter = 0u64;
    let path = &ARGS.path;
    match path.is_dir() {
        true => {
            let bar = progress_bar();
            let walker = new_walker();
            for entry in walker.filter_entry(filter_direntry).flatten() {
                if let Ok(file_data) = read_to_string(entry.path()) {
                    bar.set_message(format!("Processing {counter}/?"));
                    counter += 1;
                    reqs.push(ScanRequest {
                        kind: ScanType::Quick,
                        file_name: entry.path().to_owned(),
                        sha256sum: dirk_core::util::checksum(&file_data),
                        file_contents: None,
                        skip_cache: ARGS.skip_cache,
                    });
                }
                if reqs.len() >= ARGS.chunk_size {
                    results.append(&mut send_scan_req(reqs.drain(0..).collect()).await?.results);
                }
            }
            // Send any remaining files below ARGS.chunk_size
            results.append(&mut send_scan_req(reqs.drain(0..).collect()).await?.results);
            bar.finish();
        }
        false => {
            println!("Processing a single file");
            if path.metadata().unwrap().len() > MAX_FILESIZE {
                println!(
                    "Skipping {:?} due to size: ({})",
                    path.file_name(),
                    path.metadata().unwrap().len()
                );
            } else if let Ok(file_data) = read_to_string(path) {
                reqs.push(ScanRequest {
                    kind: ScanType::Quick,
                    file_name: path.to_owned(),
                    sha256sum: dirk_core::util::checksum(&file_data),
                    file_contents: None,
                    skip_cache: ARGS.skip_cache,
                });
                results.append(&mut send_scan_req(reqs.drain(0..).collect()).await?.results);
            }
        }
    };

    //print_quick_scan_results(results, counter);
    ScanBulkResult {
        id: Default::default(),
        results,
    }
    .print_results(ARGS.verbose);
    Ok(())
}

async fn process_input_extended() -> Result<(), reqwest::Error> {
    let mut reqs: Vec<ScanRequest> = Vec::new();
    let mut results: Vec<ScanResult> = Vec::new();
    let mut counter: u64 = 0;
    //validate_args ensures we're running in recursive mode if this is a directory, so no need to check that again here
    let path = &ARGS.path;
    match path.is_dir() {
        true => {
            let bar = progress_bar();
            let walker = new_walker();
            for entry in walker.filter_entry(filter_direntry).flatten() {
                match entry.file_type().is_file() {
                    false => continue,
                    true => {
                        if let Ok(file_req) = prep_file_request(&entry.into_path()) {
                            bar.set_message(format!("Processing {counter}/?"));
                            counter += 1;
                            reqs.push(file_req);
                        }
                    }
                }
                if reqs.len() >= ARGS.chunk_size {
                    bar.set_message(format!("Submitting {} files...", reqs.len()));
                    results.append(&mut send_scan_req(reqs.drain(0..).collect()).await?.results);
                }
            }
            bar.finish();
        }
        false => {
            println!("Processing a single file");
            if path.metadata().unwrap().len() > MAX_FILESIZE {
                println!(
                    "Skipping {:?} due to size: ({})",
                    path.file_name(),
                    path.metadata().unwrap().len()
                );
            } else if let Ok(file_req) = prep_file_request(path) {
                reqs.push(file_req);
            }
        }
    };

    results.append(&mut send_scan_req(reqs.drain(0..).collect()).await?.results);
    ScanBulkResult {
        id: Default::default(),
        results,
    }
    .print_results(ARGS.verbose);
    Ok(())
}

#[tokio::main()]
async fn main() -> Result<(), reqwest::Error> {
    validate_args();
    match ARGS.scan_type {
        ScanType::Dynamic => process_input_extended().await?,
        ScanType::FindUnknown => find_unknown_files().await?,
        ScanType::Full => process_input_extended().await?,
        ScanType::Quick => process_input_quick().await?,
    }
    Ok(())
}
