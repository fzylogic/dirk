use clap::{Parser, ValueEnum};
use dirk::dirk_api::{QuickScanBulkRequest, QuickScanBulkResult, QuickScanRequest};

use axum::http;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
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

const MAX_FILESIZE: u64 = 500_000;

#[tokio::main()]
async fn main() -> Result<(), reqwest::Error> {
    let args = Args::parse();
    let mut reqs: Vec<QuickScanRequest> = Vec::new();
    match args.check.is_dir() {
        true => match args.recursive {
            true => {
                let walker = WalkDir::new(&args.check).into_iter();
                for entry in walker.flatten() {
                    if entry.metadata().unwrap().len() > MAX_FILESIZE {
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
    println!("{url}");

    let new_post: QuickScanBulkResult = reqwest::Client::new()
        .post(url)
        .json(&QuickScanBulkRequest { requests: reqs })
        .send()
        .await?
        .json()
        .await?;
    println!("{:#?}", new_post);
    Ok(())
}
