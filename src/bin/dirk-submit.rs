use std::fs::read_to_string;

use clap::{Parser, ValueEnum};
use dirk::dirk_api::{QuickScanRequest, QuickScanResult};

use std::path::PathBuf;
use axum::http;

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

#[tokio::main()]
async fn main() -> Result<(), reqwest::Error> {
    let args = Args::parse();
    let file_data = read_to_string(&args.check).unwrap_or_else(|_| panic!("Unable to open file {}", &args.check.display()));
    let encoded = base64::encode(file_data);
    let req = QuickScanRequest {
        file_contents: encoded,
        file_name: args.check,
    };

    let urlbase = args.urlbase.unwrap_or("http://localhost:3000".parse::<http::uri::Uri>().unwrap());

    let url = match args.scan_type {
        ScanType::Full => format!("{}{}", urlbase, "scanner/full"),
        ScanType::Quick => format!("{}{}", urlbase, "scanner/quick"),
    };
    println!("{url}");

    let new_post: QuickScanResult = reqwest::Client::new()
        .post(url)
        .json(&req)
        .send()
        .await?
        .json()
        .await?;
    println!("{:#?}", new_post);
    Ok(())
}