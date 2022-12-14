use clap::{Parser, ValueEnum};
use dirk::dirk_api::{DirkResult, QuickScanBulkRequest, QuickScanBulkResult, QuickScanRequest};

use axum::http;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use axum::http::Uri;
use lazy_static::lazy_static;
use reqwest::StatusCode;
use walkdir::{DirEntry, WalkDir};

#[derive(Clone, Debug, ValueEnum)]
enum ScanType {
    Full,
    Quick,
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, value_parser)]
    check: PathBuf,
    #[clap(short, long, value_parser)]
    recursive: bool,
    #[clap(short, long)]
    verbose: bool,
    #[clap(short, long, value_parser, default_value_t = String::from("http://localhost:3000"))]
    urlbase: String,
    #[clap(long, value_enum, default_value_t=ScanType::Quick)]
    scan_type: ScanType,
}

lazy_static! {
    static ref ARGS: Args = Args::parse();
}

fn prep_file_request(path: &PathBuf, verbose: bool) -> Result<QuickScanRequest, std::io::Error> {
    let mut hasher = Sha256::new();
    let file_data = String::from_utf8_lossy(&std::fs::read(path)?).to_string();
    hasher.update(&file_data);
    let csum = base64::encode(hasher.finalize());
    let encoded = base64::encode(&file_data);
    if verbose {
        println!("Preparing request for {}", path.display());
    }
    Ok(QuickScanRequest {
        checksum: csum,
        file_contents: encoded,
        file_name: path.to_owned(),
    })
}

const MAX_FILESIZE: u64 = 500_000; // 500kb max file size to scan

fn print_results(results: QuickScanBulkResult) {
    let result_count = results.results.len();
    let mut bad_count: usize = 0;
    for result in results.results {
        match result.result {
            DirkResult::OK => {
                if ARGS.verbose {
                    println!("{:?} passed", result.file_name)
                }
            },
            DirkResult::Inconclusive => {
                println!("{:?} was inconclusive", result.file_name)
            },
            DirkResult::Bad => {
                println!("{:?} is BAD: {}", result.file_name, result.reason);
                bad_count += 1;
            }
        }
    }
    println!("Summary: Out of {} files checked, {} were bad", result_count, bad_count);
}

fn validate_args() {
    match ARGS.check.is_dir() {
        true => match ARGS.recursive {
            true => return,
            false => {
                panic!("Can't check a directory w/o specifying --recursive");
            }
        },
        false => return,
    }
}

fn filter_direntry(entry: &DirEntry) -> bool {
    if entry.metadata().unwrap().len() > MAX_FILESIZE {
        if ARGS.verbose{
            println!(
                "Skipping {:?} due to size: ({})",
                &ARGS.check.file_name(),
                &ARGS.check.metadata().unwrap().len()
            );
        }
        return false;
    }
    true
}

#[tokio::main()]
async fn main() -> Result<(), reqwest::Error> {
    let mut reqs: Vec<QuickScanRequest> = Vec::new();
    validate_args();
    match ARGS.check.is_dir() {
        true => match ARGS.recursive {
            true => {
                let walker = WalkDir::new(&ARGS.check).follow_links(false).into_iter();
                for entry in walker.filter_entry(|e| filter_direntry(e)).flatten() {
                    match entry.file_type().is_file() {
                        false => continue,
                        true => {
                            if let Ok(file_req) = prep_file_request(&entry.into_path(), ARGS.verbose) {
                                reqs.push(file_req);
                            }
                        }
                    }
                }
            }
            false => {
                eprintln!("Can't process a directory without being passed --recursive");
                std::process::exit(1);
            }
        },
        false => {
            println!("Processing a single file");
            if ARGS.check.metadata().unwrap().len() > MAX_FILESIZE {
                println!(
                    "Skipping {:?} due to size: ({})",
                    &ARGS.check.file_name(),
                    &ARGS.check.metadata().unwrap().len()
                );
            } else if let Ok(file_req) = prep_file_request(&ARGS.check, ARGS.verbose) {
                reqs.push(file_req);
            }
        }
    };

    let urlbase: Uri = ARGS.urlbase.parse::<http::uri::Uri>().unwrap();

    let url = match ARGS.scan_type {
        ScanType::Full => format!("{}{}", urlbase, "scanner/full"),
        ScanType::Quick => format!("{}{}", urlbase, "scanner/quick"),
    };

    let resp = reqwest::Client::new()
        .post(url)
        .json(&QuickScanBulkRequest { requests: reqs })
        .send()
        .await?;
    //println!("Received status: {}", resp.status().as_str());
    match resp.status() {
        StatusCode::OK => {
            //println!("{}", resp.text().await.unwrap());
            let new_post: QuickScanBulkResult =
                resp
                    .json()
                    .await
                    .unwrap();
            println!("{:#?}", new_post);
            print_results(new_post);
        },
        _ => {
            println!("Received unexpected response code: {}", resp.status().as_str());
        }
    }

    Ok(())
}
