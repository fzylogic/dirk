use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    path: PathBuf,
}

fn main() {
    let args = Args::parse();
    let file_data = String::from_utf8_lossy(&std::fs::read(args.path).unwrap()).to_string();
    let csum = dirk_core::util::checksum(&file_data);
    println!("{}", csum);
}
