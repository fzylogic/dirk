use clap::{Args, Parser, Subcommand};

use dirk_core::entities::*;
use dirk_core::errors::*;
use std::collections::HashSet;
use std::fs::read_to_string;

use axum::http::Uri;
use base64::{engine::general_purpose, Engine as _};
use dirk_core::entities::sea_orm_active_enums::*;
use dirk_core::models::dirk::{FileUpdateRequest, SubmissionType};
use dirk_core::models::*;
use dirk_core::util::MAX_FILESIZE;
use indicatif::{ProgressBar, ProgressStyle};
use lazy_static::lazy_static;
use reqwest::StatusCode;
use std::path::PathBuf;
use std::time::Duration;
use walkdir::{IntoIter, WalkDir};

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
    #[command(subcommand)]
    command: Commands,
}

#[derive(Args, Clone)]
struct Scan {
    #[clap(long, default_value_t = 500)]
    chunk_size: usize,
    #[clap(long)]
    skip_cache: bool,
    #[clap(value_enum)]
    scan_type: dirk::ScanType,
    #[clap(value_parser)]
    path: PathBuf,
}

#[derive(Args, Clone)]
struct Submit {
    #[clap(short, long, value_enum, default_value_t = SubmissionType::Update)]
    action: SubmissionType,
    #[clap(short, long, value_enum)]
    file_class: Option<FileStatus>,
    #[clap(value_parser)]
    path: PathBuf,
}

/// Takes a path to a file or directory and turns it into a scan request
fn prep_file_request(path: &PathBuf) -> Result<dirk::ScanRequest, DirkError> {
    let file_data = String::from_utf8_lossy(&std::fs::read(path)?).to_string();
    let csum = dirk_core::util::checksum(&file_data);
    let options = scan_options().expect("No scanner options passed");
    let encoded = general_purpose::STANDARD.encode(&file_data);
    if ARGS.verbose {
        println!("Preparing request for {}", path.display());
    }
    Ok(dirk::ScanRequest {
        sha1sum: csum,
        kind: options.scan_type.clone(),
        file_contents: Some(encoded),
        file_name: path.to_owned(),
        skip_cache: options.skip_cache,
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
async fn send_scan_req(reqs: Vec<dirk::ScanRequest>) -> Result<dirk::ScanBulkResult, DirkError> {
    let urlbase: Uri = ARGS
        .urlbase
        .parse::<Uri>()
        .expect("Unable to parse urlbase arg into a URI");
    let options = scan_options().unwrap();
    let url = options.scan_type.url(urlbase);

    let resp = reqwest::Client::new()
        .post(url)
        .json(&dirk::ScanBulkRequest {
            requests: reqs.clone(),
        })
        .send()
        .await?;
    match resp.status() {
        StatusCode::OK => {}
        _ => {
            eprintln!("Received non-OK status: {}", resp.status())
        }
    }
    let resp_data = resp.json().await?;
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
        if let Ok(file_data) = read_to_string(entry.path()) {
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
    WalkDir::new(path).follow_links(false).into_iter()
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

/// Quick scan
async fn process_input_quick() -> Result<(), DirkError> {
    let mut reqs: Vec<dirk::ScanRequest> = Vec::new();
    let mut results: Vec<dirk::ScanResult> = Vec::new();
    let mut counter = 0u64;
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
                if let Ok(file_data) = read_to_string(entry.path()) {
                    bar.set_message(format!("Processing {counter}/?"));
                    counter += 1;
                    reqs.push(dirk::ScanRequest {
                        kind: dirk::ScanType::Quick,
                        file_name: entry.path().to_owned(),
                        sha1sum: dirk_core::util::checksum(&file_data),
                        file_contents: None,
                        skip_cache: options.skip_cache,
                    });
                }
                if reqs.len() >= options.chunk_size {
                    results.append(&mut send_scan_req(reqs.drain(0..).collect()).await?.results);
                }
            }
            // Send any remaining files below ARGS.chunk_size
            results.append(&mut send_scan_req(reqs.drain(0..).collect()).await?.results);
            bar.finish();
        }
        false => {
            println!("Processing a single file");
            if let Ok(md) = path.metadata() {
                let size = md.len();
                if size > MAX_FILESIZE {
                    println!("Skipping {:?} due to size: ({})", path.file_name(), size);
                } else if let Ok(file_data) = read_to_string(&path) {
                    reqs.push(dirk::ScanRequest {
                        kind: dirk::ScanType::Quick,
                        file_name: path.to_owned(),
                        sha1sum: dirk_core::util::checksum(&file_data),
                        file_contents: None,
                        skip_cache: options.skip_cache,
                    });
                    results.append(&mut send_scan_req(reqs.drain(0..).collect()).await?.results);
                }
            } else {
                eprintln!("unable to fetch metadata for {}", path.display());
            }
        }
    };

    dirk::ScanBulkResult {
        id: Default::default(),
        results,
    }
    .print_results(ARGS.verbose);
    Ok(())
}

/// Full and Dynamic scans
async fn process_input_extended() -> Result<(), DirkError> {
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
                            bar.set_message(format!("Processing {counter}/?"));
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
    dirk::ScanBulkResult {
        id: Default::default(),
        results,
    }
    .print_results(ARGS.verbose);
    Ok(())
}
async fn list_known_files() -> Result<(), DirkError> {
    let urlbase: Uri = ARGS.urlbase.parse::<Uri>()?;
    let resp = reqwest::Client::new()
        .get(format!("{}{}", urlbase, "files/list"))
        .send()
        .await?;

    let file_data: Vec<files::Model> = resp.json().await?;
    for file in file_data.into_iter() {
        println!("File ID: {}", file.id);
        println!("  File SHA1: {}", file.sha1sum);
        println!("  File First Seen: {}", file.first_seen);
        println!("  File Last Seen: {}", file.last_seen);
        println!("  File Last Updated: {}", file.last_updated);
        println!("  File Status: {:?}", file.file_status);
    }
    Ok(())
}

async fn update_file() -> Result<(), DirkError> {
    let path = path();
    let file_data = String::from_utf8_lossy(&std::fs::read(path)?).to_string();
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
            dirk::ScanType::Dynamic => process_input_extended().await?,
            dirk::ScanType::FindUnknown => find_unknown_files().await?,
            dirk::ScanType::Full => process_input_extended().await?,
            dirk::ScanType::Quick => process_input_quick().await?,
        },
        Commands::Submit(args) => match args.action {
            SubmissionType::List => list_known_files().await.unwrap(),
            SubmissionType::Update => update_file().await.unwrap(),
        },
    }
    Ok(())
}
