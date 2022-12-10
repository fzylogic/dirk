use std::fs::read_to_string;
use base64;
use clap::Parser;
use dirk::dirk_api::{QuickScanResult, QuickScanRequest, Result, Reason};

use std::path::PathBuf;
use axum::http;
use tokio;
use walkdir::WalkDir;

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
    url: http::uri::Uri,
}

#[tokio::main()]
async fn main() {
    let args = Args::parse();
    let client = reqwest::Client::new();
    let file_data = read_to_string(&args.check).expect(format!("Unable to open file {}", &args.check.display()).as_str());
    let encoded = base64::encode(file_data);
}