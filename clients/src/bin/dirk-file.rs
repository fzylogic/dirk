use clap::{Parser, ValueEnum};
//use std::fs::File;

use axum::http::Uri;

use dirk_core::entities::*;
use dirk_core::models::dirk::FileUpdateRequest;
//use flate2::read::GzDecoder;
use lazy_static::lazy_static;
use std::path::PathBuf;
//use tar::Archive;

use dirk_core::entities::sea_orm_active_enums::*;

#[derive(Clone, Debug, ValueEnum)]
enum Action {
    List,
    Update,
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, value_enum, default_value_t = Action::Update)]
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

    let file_data: Vec<files::Model> = resp.json().await?;
    for file in file_data.into_iter() {
        println!("File ID: {}", file.id);
        println!("  File SHA256: {}", file.sha256sum);
        println!("  File First Seen: {}", file.first_seen);
        println!("  File Last Seen: {}", file.last_seen);
        println!("  File Last Updated: {}", file.last_updated);
        println!("  File Status: {:?}", file.file_status);
    }
    Ok(())
}

async fn update_file() -> Result<(), reqwest::Error> {
    let path = ARGS.path.as_ref().unwrap();
    let file_data = String::from_utf8_lossy(&std::fs::read(path).unwrap()).to_string();
    let csum = dirk_core::util::checksum(&file_data);
    let req = FileUpdateRequest {
        file_status: ARGS.file_class.unwrap(),
        checksum: csum,
    };
    let urlbase: Uri = ARGS.urlbase.parse::<Uri>().unwrap();
    let url = format!("{}{}", urlbase, "files/update");
    let resp = reqwest::Client::new().post(url).json(&req).send().await?;
    println!("{:#?}", resp.status());
    Ok(())
}

/*async fn whitelist_archive() -> Result<(), std::io::Error> {
    let path = ARGS.path.as_ref().unwrap();
    let _file = File::open(path)?;
    Ok(())
}*/

#[allow(unreachable_patterns)]
#[tokio::main()]
async fn main() -> Result<(), reqwest::Error> {
    match ARGS.action {
        Action::Update => {
            update_file().await?;
        }
        Action::List => {
            list_known_files().await?;
        }
        _ => {
            todo!();
        }
    }
    Ok(())
}
