use std::collections::HashSet;
use std::fs::read;
use std::path::PathBuf;
use std::time::Duration;

use axum::http::Uri;
use base64::{engine::general_purpose, Engine as _};
use clap::{Args, Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use lazy_static::lazy_static;
use reqwest::StatusCode;
use walkdir::{IntoIter, WalkDir};

use dirk_core::entities::sea_orm_active_enums::*;
use dirk_core::entities::*;
use dirk_core::errors::*;
use dirk_core::models::dirk::{FileUpdateRequest, ScanBulkResult, ScanType};
use dirk_core::models::*;
use dirk_core::util::MAX_FILESIZE;

lazy_static! {
    static ref ARGS: Cli = Cli::parse();
}

fn scan_options() -> Option<Scan> {
    match ARGS.command.clone() {
        Commands::Scan(scan) => Some(scan),
        _ => None,
    }
}

fn submit_options() -> Option<Submit> {
    match ARGS.command.clone() {
        Commands::Submit(submit) => Some(submit),
        _ => None,
    }
}

fn path() -> PathBuf {
    match &ARGS.command {
        Commands::Scan(scan) => &scan.path,
        Commands::Submit(submit) => &submit.path,
    }
    .clone()
}

#[derive(Clone, Subcommand)]
enum Commands {
    /// Scan files
    Scan(Scan),
    /// Submit files
    Submit(Submit),
}

#[derive(Clone, Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(short, long, value_parser)]
    recursive: bool,
    #[clap(short, long)]
    verbose: bool,
    #[clap(short, long, value_parser, default_value_t = String::from("http://localhost:3000"))]
    urlbase: String,
    #[clap(short, long)]
    follow_symlinks: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Args, Clone)]
struct Scan {
    #[clap(long, default_value_t = 50)]
    chunk_size: usize,
    #[clap(long)]
    skip_cache: bool,
    #[clap(value_enum)]
    scan_type: ScanType,
    #[clap(value_parser)]
    path: PathBuf,
}

#[derive(Args, Clone)]
struct Submit {
    #[clap(short, long, value_enum)]
    file_class: Option<FileStatus>,
    #[clap(value_parser)]
    path: PathBuf,
}

/// Takes a path to a file or directory and turns it into a scan request
fn prep_file_request(path: &PathBuf) -> Result<dirk::ScanRequest, DirkError> {
    let file_data = &std::fs::read(path)?;
    let csum = dirk_core::util::checksum(file_data);
    let options = scan_options().expect("No scanner options found");
    if ARGS.verbose {
        println!(
            "Preparing request for {} ({} bytes)",
            path.display(),
            path.metadata().unwrap().len()
        );
    }
    Ok(dirk::ScanRequest {
        sha1sum: csum,
        kind: options.scan_type.clone(),
        file_contents: match options.scan_type {
            ScanType::Dynamic | ScanType::Full => Some(general_purpose::STANDARD.encode(file_data)),
            _ => None,
        },
        file_name: path.to_owned(),
    })
}

/// Quickly validate that arguments we were passed
fn validate_args() {
    let path = match &ARGS.command {
        Commands::Scan(_) => scan_options().unwrap().path,
        Commands::Submit(_) => submit_options().unwrap().path,
    };
    match &path.is_dir() {
        true => match ARGS.recursive {
            true => (),
            false => {
                panic!("Can't check a directory w/o specifying --recursive");
            }
        },
        false => (),
    }
}

/// Take a vector of scan requests and send them to our API
async fn send_scan_req(reqs: Vec<dirk::ScanRequest>) -> Result<ScanBulkResult, DirkError> {
    let urlbase: Uri = ARGS
        .urlbase
        .parse::<Uri>()
        .expect("Unable to parse urlbase arg into a URI");
    let options = scan_options().unwrap();
    let url = options.scan_type.url(urlbase);
    if ARGS.verbose {
        println!("Sending {} requests", reqs.len());
    }
    let resp = match reqwest::Client::new()
        .post(url)
        .json(&dirk::ScanBulkRequest {
            requests: reqs.clone(),
            skip_cache: options.skip_cache,
        })
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return Err(DirkError::from(e));
        }
    };

    match resp.status() {
        StatusCode::OK => {}
        _ => {
            eprintln!("Received non-OK status: {}", resp.status())
        }
    }

    let resp_data: ScanBulkResult = resp.json().await?;
    if ARGS.verbose {
        println!("Received {} results", resp_data.results.len());
    }
    Ok(resp_data)
}

/// Find and report on files whose sha1sums don't match any known files
async fn find_unknown_files() -> Result<(), DirkError> {
    let urlbase: Uri = ARGS
        .urlbase
        .parse::<Uri>()
        .expect("Unable to parse urlbase arg into a URI");
    let resp = reqwest::Client::new()
        .get(format!("{}{}", urlbase, "files/list"))
        .send()
        .await?;
    //let mut known_files = HashSet::new();
    let file_data: Vec<files::Model> = resp.json().await?;
    //The sha1 column is a unique key in the database, so no need to check if the hash entry already exists
    let mut known_files: HashSet<String> = HashSet::new();
    file_data.into_iter().for_each(|file| {
        known_files.insert(file.sha1sum);
    });
    let walker = new_walker();
    for entry in walker
        .filter_entry(dirk_core::util::filter_direntry)
        .flatten()
    {
        if let Ok(file_data) = read(entry.path()) {
            if !known_files.contains(dirk_core::util::checksum(&file_data).as_str()) {
                println!("{}", entry.path().display());
            }
        }
    }
    Ok(())
}

/// Returns a fresh WalkDir object
fn new_walker() -> IntoIter {
    let path = path();
    WalkDir::new(path)
        .follow_links(ARGS.follow_symlinks)
        .into_iter()
}

/// Initialize the progress bar used in Full and Dynamic scans
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

/// Full and Dynamic scans
async fn process_input() -> Result<(), DirkError> {
    let mut reqs: Vec<dirk::ScanRequest> = Vec::new();
    let mut results: Vec<dirk::ScanResult> = Vec::new();
    let mut counter: u64 = 0;
    //validate_args ensures we're running in recursive mode if this is a directory, so no need to check that again here
    let options = scan_options().unwrap();
    let path = path();
    match path.is_dir() {
        true => {
            let bar = progress_bar();
            let walker = new_walker();
            for entry in walker
                .filter_entry(dirk_core::util::filter_direntry)
                .flatten()
            {
                match entry.file_type().is_file() {
                    false => continue,
                    true => {
                        if let Ok(file_req) = prep_file_request(&entry.into_path()) {
                            bar.set_message(format!("Processing {}/?", counter));
                            counter += 1;
                            reqs.push(file_req);
                        }
                    }
                }
                if reqs.len() >= options.chunk_size {
                    bar.set_message(format!("Submitting {} files...", reqs.len()));
                    results.append(&mut send_scan_req(reqs.drain(0..).collect()).await?.results);
                }
            }
            bar.finish();
        }
        false => {
            println!("Processing a single file");
            if let Ok(md) = path.metadata() {
                let size = md.len();
                if size > MAX_FILESIZE {
                    println!("Skipping {:?} due to size: ({})", path.file_name(), size);
                } else if let Ok(file_req) = prep_file_request(&path) {
                    reqs.push(file_req);
                }
            } else {
                eprintln!("unable to fetch metadata for {}", path.display());
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
// async fn list_known_files() -> Result<(), DirkError> {
//     let urlbase: Uri = ARGS.urlbase.parse::<Uri>()?;
//     let resp = reqwest::Client::new()
//         .get(format!("{}{}", urlbase, "files/list"))
//         .send()
//         .await?;
//
//     let file_data: Vec<files::Model> = resp.json().await?;
//     for file in file_data.into_iter() {
//         println!("File ID: {}", file.id);
//         println!("  File SHA1: {}", file.sha1sum);
//         println!("  File First Seen: {}", file.first_seen);
//         println!("  File Last Seen: {}", file.last_seen);
//         println!("  File Last Updated: {}", file.last_updated);
//         println!("  File Status: {:?}", file.file_status);
//     }
//     Ok(())
// }

async fn update_file() -> Result<(), DirkError> {
    let path = path();
    let file_data = read(path)?;
    update_file_data(file_data).await
}

// #[tokio::test]
// async fn test_update_file_data() {
//     let _ = update_file_data("I am a good file".to_string()).await;
// }

async fn update_file_data(file_data: Vec<u8>) -> Result<(), DirkError> {
    let csum = dirk_core::util::checksum(&file_data);
    let options = submit_options().unwrap();
    let req = FileUpdateRequest {
        file_status: options.file_class.ok_or(DirkError::ArgumentError)?,
        checksum: csum,
    };
    let urlbase: Uri = ARGS.urlbase.parse::<Uri>()?;
    let url = format!("{}{}", urlbase, "files/update");
    let resp = reqwest::Client::new().post(url).json(&req).send().await?;
    println!("{:#?}", resp.status());
    Ok(())
}

#[tokio::main()]
async fn main() -> Result<(), DirkError> {
    validate_args();
    match &ARGS.command {
        Commands::Scan(args) => match args.scan_type {
            ScanType::Dynamic => process_input().await?,
            ScanType::FindUnknown => find_unknown_files().await?,
            ScanType::Full => process_input().await?,
            ScanType::Quick => process_input().await?,
        },
        Commands::Submit(_args) => update_file().await.unwrap(),
    }
    Ok(())
}
