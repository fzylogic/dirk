use clap::{Parser, ValueEnum};
use dirk::dirk_api::{DirkResult, QuickScanBulkRequest, QuickScanBulkResult, QuickScanRequest};

use axum::http;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use reqwest::StatusCode;
use walkdir::WalkDir;

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
    #[clap(short, long, value_parser)]
    urlbase: Option<http::uri::Uri>,
    #[clap(long, value_enum, default_value_t=ScanType::Quick)]
    scan_type: ScanType,
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

fn print_results(results: QuickScanBulkResult, verbose: bool) {
    let result_count = results.results.len();
    let mut bad_count: usize = 0;
    for result in results.results {
        match result.result {
            DirkResult::OK => {
                if verbose {
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

#[tokio::main()]
async fn main() -> Result<(), reqwest::Error> {
    let args = Args::parse();
    let mut reqs: Vec<QuickScanRequest> = Vec::new();
    match args.check.is_dir() {
        true => match args.recursive {
            true => {
                let walker = WalkDir::new(&args.check).into_iter();
                for entry in walker.flatten() {
                    if entry.metadata().unwrap().len() > MAX_FILESIZE && entry.metadata().unwrap().len() == 0 {
                        println!(
                            "Skipping {:?} due to size: ({})",
                            &entry.file_name(),
                            &entry.metadata().unwrap().len()
                        );
                        continue;
                    }
                    match entry.file_type().is_file() {
                        false => continue,
                        true => {
                            if let Ok(file_req) = prep_file_request(&entry.into_path(), args.verbose) {
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
            if args.check.metadata().unwrap().len() > MAX_FILESIZE {
                println!(
                    "Skipping {:?} due to size: ({})",
                    &args.check.file_name(),
                    &args.check.metadata().unwrap().len()
                );
            } else if let Ok(file_req) = prep_file_request(&args.check, args.verbose) {
                reqs.push(file_req);
            }
        }
    };

    let urlbase = args
        .urlbase
        .unwrap_or_else(|| "http://localhost:3000".parse::<http::uri::Uri>().unwrap());

    let url = match args.scan_type {
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
            let new_post: QuickScanBulkResult =
                resp
                    .json()
                    .await?;
            //println!("{:#?}", new_post);
            print_results(new_post, args.verbose);
        },
        _ => {
            println!("Received unexpected response code: {}", resp.status().as_str());
        }
    }

    Ok(())
}
