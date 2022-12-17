
use clap::{Parser, ValueEnum};
use dirk::dirk_api::{FileUpdateRequest};

use axum::http::Uri;

use lazy_static::lazy_static;
use std::path::PathBuf;

use dirk::entities::sea_orm_active_enums::FileStatus;

#[derive(Clone, Debug, ValueEnum)]
enum Action {
    List,
    Update,
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, value_enum)]
    action: Action,
    #[clap(short, long, value_enum)]
    file_class: Option<FileStatus>,
    #[clap(short, long, value_parser)]
    path: Option<PathBuf>,
    #[clap(short, long, value_parser)]
    recursive: bool,
    #[clap(short, long)]
    verbose: bool,
    #[clap(short, long, value_parser, default_value_t = String::from("http://localhost:3000"))]
    urlbase: String,
}

lazy_static! {
    static ref ARGS: Args = Args::parse();
}

async fn list_known_files() -> Result<(), reqwest::Error> {
    let urlbase: Uri = ARGS.urlbase.parse::<Uri>().unwrap();
    let resp = reqwest::Client::new()
        .get(format!("{}{}", urlbase, "files/list"))
        .send()
        .await?;
    println!("{:?}", resp);
    println!("{}", resp.text().await?);
    Ok(())
}

async fn update_file() -> Result<(), reqwest::Error> {
    let path = ARGS.path.as_ref().unwrap();
    let file_data = String::from_utf8_lossy(&std::fs::read(path).unwrap()).to_string();
    let csum = dirk::util::checksum(&file_data);
    let req = FileUpdateRequest {
        file_status: ARGS.file_class.unwrap(),
        checksum: csum,
    };
    let urlbase: Uri = ARGS.urlbase.parse::<Uri>().unwrap();
    let url = format!("{}{}", urlbase, "files/update");
    let resp = reqwest::Client::new()
        .post(url)
        .json(&req)
        .send()
        .await?;
    println!("{:?}", resp);
    Ok(())
}

#[allow(unreachable_patterns)]
#[tokio::main()]
async fn main() -> Result<(), reqwest::Error> {
    match ARGS.action {
        Action::Update => {
            update_file().await?;
        },
        Action::List => {
            list_known_files().await?;
        },
        _ => {
            todo!();
        }
    }
    Ok(())
}
