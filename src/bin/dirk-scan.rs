use clap::{Parser, ValueEnum};
use dirk::dirk_api::{
    DirkResult, FullScanBulkRequest, FullScanBulkResult, FullScanRequest, QuickScanBulkRequest,
    QuickScanBulkResult, QuickScanRequest,
};
use std::fs::read_to_string;

use axum::http::Uri;
use dirk::entities::sea_orm_active_enums::FileStatus;
use indicatif::{ProgressBar, ProgressStyle};
use lazy_static::lazy_static;
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
    #[clap(long, default_value_t = 500)]
    chunk_size: usize,
    #[clap(short, long, value_parser)]
    path: Option<PathBuf>,
    #[clap(short, long, value_parser)]
    recursive: bool,
    #[clap(long, value_enum, default_value_t=ScanType::Quick)]
    scan_type: ScanType,
    #[clap(short, long)]
    verbose: bool,
    #[clap(short, long, value_parser, default_value_t = String::from("http://localhost:3000"))]
    urlbase: String,
}

lazy_static! {
    static ref ARGS: Args = Args::parse();
}

fn prep_file_request(path: &PathBuf) -> Result<FullScanRequest, std::io::Error> {
    let file_data = String::from_utf8_lossy(&std::fs::read(path)?).to_string();
    let csum = dirk::util::checksum(&file_data);
    let encoded = base64::encode(&file_data);
    if ARGS.verbose {
        println!("Preparing request for {}", path.display());
    }
    Ok(FullScanRequest {
        checksum: csum,
        file_contents: encoded,
        file_name: path.to_owned(),
    })
}

const MAX_FILESIZE: u64 = 500_000; // 500kb max file size to scan

fn print_quick_scan_results(results: Vec<QuickScanBulkResult>) {
    let mut result_count: usize = 0;
    let mut bad_count: usize = 0;
    for bulk_result in results {
        result_count += bulk_result.results.len();
        for result in bulk_result.results {
            match result.result {
                FileStatus::Good | FileStatus::Whitelisted => {
                    if ARGS.verbose {
                        println!("{:?} passed", result.sha256sum)
                    }
                }
                FileStatus::Bad | FileStatus::Blacklisted => {
                    println!("{:?} is BAD", result.sha256sum);
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

fn print_full_scan_results(results: Vec<FullScanBulkResult>) {
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
    match &ARGS.path {
        Some(path) => match path.is_dir() {
            true => match ARGS.recursive {
                true => (),
                false => {
                    panic!("Can't check a directory w/o specifying --recursive");
                }
            },
            false => (),
        },
        _ => {}
    }
}

fn filter_direntry(entry: &DirEntry) -> bool {
    if let Some(path) = &ARGS.path {
        if entry.metadata().unwrap().len() > MAX_FILESIZE {
            if ARGS.verbose {
                println!(
                    "Skipping {:?} due to size: ({})",
                    &path.file_name(),
                    &path.metadata().unwrap().len()
                );
            }
            return false;
        }
    }
    true
}

async fn send_full_scan_req(
    reqs: Vec<FullScanRequest>,
) -> Result<FullScanBulkResult, reqwest::Error> {
    let urlbase: Uri = ARGS.urlbase.parse::<Uri>().unwrap();
    let url = match ARGS.scan_type {
        ScanType::Full => format!("{}{}", urlbase, "scanner/full"),
        ScanType::Quick => format!("{}{}", urlbase, "scanner/quick"),
    };

    let resp = reqwest::Client::new()
        .post(url)
        .json(&FullScanBulkRequest { requests: reqs })
        .send()
        .await
        .unwrap();
    let new_post: FullScanBulkResult = resp.json().await?;
    Ok(new_post)
}

async fn send_quick_scan_req(
    reqs: Vec<QuickScanRequest>,
) -> Result<QuickScanBulkResult, reqwest::Error> {
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

async fn process_input_quick() -> Result<(), reqwest::Error> {
    let mut reqs: Vec<QuickScanRequest> = Vec::new();
    let mut results: Vec<QuickScanBulkResult> = Vec::new();
    //validate_args ensures we're running in recursive mode if this is a directory, so no need to check that again here
    if let Some(path) = &ARGS.path {
        match path.is_dir() {
            true => {
                let walker = WalkDir::new(path).follow_links(false).into_iter();
                for entry in walker.filter_entry(filter_direntry).flatten() {
                    match entry.file_type().is_file() {
                        false => continue,
                        true => {
                            if let Ok(file_data) = read_to_string(entry.path()) {
                                reqs.push(QuickScanRequest {
                                    sha256sum: dirk::util::checksum(&file_data),
                                });
                            }
                        }
                    }
                    if reqs.len() >= ARGS.chunk_size {
                        results.push(send_quick_scan_req(reqs.drain(1..).collect()).await?);
                    }
                }
                // Send any remaining files below ARGS.chunk_size
                results.push(send_quick_scan_req(reqs.drain(1..).collect()).await?);
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
                    reqs.push(QuickScanRequest {
                        sha256sum: dirk::util::checksum(&file_data),
                    });
                    results.push(send_quick_scan_req(reqs.drain(1..).collect()).await?);
                }
            }
        };
    }
    print_quick_scan_results(results);
    Ok(())
}

async fn process_input_full() -> Result<(), reqwest::Error> {
    let mut reqs: Vec<FullScanRequest> = Vec::new();
    let mut results: Vec<FullScanBulkResult> = Vec::new();
    let mut counter: u64 = 0;
    //validate_args ensures we're running in recursive mode if this is a directory, so no need to check that again here
    if let Some(path) = &ARGS.path {
        match path.is_dir() {
            true => {
                let bar = ProgressBar::new_spinner();
                bar.enable_steady_tick(Duration::from_millis(200));
                bar.set_style(
                    ProgressStyle::with_template("{spinner} [{elapsed}] {msg}")
                        .unwrap()
                        .tick_strings(&["🌑", "🌘", "🌗", "🌖", "🌕", "🌔", "🌓", "🌒", "🌑"]),
                );
                let walker = WalkDir::new(path).follow_links(false).into_iter();
                for entry in walker.filter_entry(filter_direntry).flatten() {
                    bar.set_message(format!("Processing {counter}/?"));
                    counter += 1;
                    match entry.file_type().is_file() {
                        false => continue,
                        true => {
                            if let Ok(file_req) = prep_file_request(&entry.into_path()) {
                                reqs.push(file_req);
                            }
                        }
                    }
                    if reqs.len() >= ARGS.chunk_size {
                        bar.set_message(format!("Submitting {} files...", reqs.len()));
                        results.push(send_full_scan_req(reqs.drain(1..).collect()).await?);
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

        results.push(send_full_scan_req(reqs.drain(1..).collect()).await?);
        print_full_scan_results(results);
    }
    Ok(())
}

#[tokio::main()]
async fn main() -> Result<(), reqwest::Error> {
    validate_args();
    match ARGS.scan_type {
        ScanType::Quick => process_input_quick().await?,
        ScanType::Full => process_input_full().await?,
    }
    Ok(())
}
