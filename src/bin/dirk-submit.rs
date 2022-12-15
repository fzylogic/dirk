use clap::{Parser, ValueEnum};
use dirk::dirk_api::{DirkResult, QuickScanBulkRequest, QuickScanBulkResult, QuickScanRequest};

use axum::http::Uri;
use indicatif::{ProgressBar, ProgressStyle};
use lazy_static::lazy_static;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::Duration;
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
    #[clap(long, default_value_t = 500)]
    chunk_size: usize,
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

fn prep_file_request(path: &PathBuf) -> Result<QuickScanRequest, std::io::Error> {
    let mut hasher = Sha256::new();
    let file_data = String::from_utf8_lossy(&std::fs::read(path)?).to_string();
    hasher.update(&file_data);
    let csum = base64::encode(hasher.finalize());
    let encoded = base64::encode(&file_data);
    if ARGS.verbose {
        println!("Preparing request for {}", path.display());
    }
    Ok(QuickScanRequest {
        checksum: csum,
        file_contents: encoded,
        file_name: path.to_owned(),
    })
}

const MAX_FILESIZE: u64 = 500_000; // 500kb max file size to scan

fn print_results(results: Vec<QuickScanBulkResult>) {
    let mut result_count: usize = 0;
    let mut bad_count: usize = 0;
    for bulk_result in results {
        result_count += bulk_result.results.len();
        for result in bulk_result.results {
            match result.result {
                DirkResult::OK => {
                    if ARGS.verbose {
                        println!("{:?} passed", result.file_name)
                    }
                }
                DirkResult::Inconclusive => {
                    println!("{:?} was inconclusive", result.file_name)
                }
                DirkResult::Bad => {
                    println!("{:?} is BAD: {}", result.file_name, result.reason);
                    bad_count += 1;
                }
            }
        }
    }
    println!(
        "Summary: Out of {} files checked, {} were bad",
        result_count, bad_count
    );
}

fn validate_args() {
    match ARGS.check.is_dir() {
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
    if entry.metadata().unwrap().len() > MAX_FILESIZE {
        if ARGS.verbose {
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

async fn send_req(reqs: Vec<QuickScanRequest>) -> Result<QuickScanBulkResult, reqwest::Error> {
    let urlbase: Uri = ARGS.urlbase.parse::<Uri>().unwrap();
    let url = match ARGS.scan_type {
        ScanType::Full => format!("{}{}", urlbase, "scanner/full"),
        ScanType::Quick => format!("{}{}", urlbase, "scanner/quick"),
    };

    let resp = reqwest::Client::new()
        .post(url)
        .json(&QuickScanBulkRequest { requests: reqs })
        .send()
        .await
        .unwrap();
    let new_post: QuickScanBulkResult = resp.json().await?;
    Ok(new_post)
}

async fn process_input() -> Result<(), reqwest::Error> {
    let mut reqs: Vec<QuickScanRequest> = Vec::new();
    let mut results: Vec<QuickScanBulkResult> = Vec::new();
    let mut counter: u64 = 0;
    match ARGS.check.is_dir() {
        true => match ARGS.recursive {
            true => {
                let bar = ProgressBar::new_spinner();
                bar.enable_steady_tick(Duration::from_millis(200));
                bar.set_style(
                    ProgressStyle::with_template("{spinner:.blue/blue} [{elapsed}] {msg}")
                        .unwrap()
                        .tick_strings(&[
                            "🌘",
                            "🌗",
                            "🌖",
                            "🌕",
                            "🌔",
                            "🌓",
                            "🌒",
                            "🌑",
                            "🌑",
                        ]),
                );
                let walker = WalkDir::new(&ARGS.check).follow_links(false).into_iter();
                for entry in walker.filter_entry(filter_direntry).flatten() {
                    bar.set_message(format!("Processing {counter}/?"));
                    counter += 1;
                    match entry.file_type().is_file() {
                        false => continue,
                        true => {
                            if let Ok(file_req) = prep_file_request(&entry.into_path()) {
                                //bar.tick();
                                //println!("tick");
                                reqs.push(file_req);
                            }
                        }
                    }
                    if reqs.len() >= ARGS.chunk_size {
                        bar.set_message(format!("Submitting {} files...", reqs.len()));
                        results.push(send_req(reqs.drain(1..).collect()).await?);
                    }
                }
                bar.finish();
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
            } else if let Ok(file_req) = prep_file_request(&ARGS.check) {
                reqs.push(file_req);
            }
        }
    };
    results.push(send_req(reqs.drain(1..).collect()).await?);
    print_results(results);
    Ok(())
}

#[tokio::main()]
async fn main() -> Result<(), reqwest::Error> {
    validate_args();

    process_input().await?;

    Ok(())
}
