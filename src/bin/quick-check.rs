use std::iter::Scan;
use clap::Parser;
use dirk::hank::{build_sigs_from_file, ResultStatus, ScanResult};
use std::path::PathBuf;
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, value_parser)]
    check: PathBuf,
    #[clap(short, long, value_parser)]
    recursive: bool,
    #[clap(short, long, value_parser, default_value_t = String::from("signatures.json"))]
    signatures: String,
    #[clap(short, long)]
    verbose: bool,
}

fn print_result(result: &ScanResult, verbose: bool) {
    match result.status {
        ResultStatus::OK => {
            if !verbose {
                return;
            }
        },
        _ => {}
    }
    println!("{} {}", result.filename.display(), result.status);
}

fn main() {
    let args = Args::parse();
    let sigs = build_sigs_from_file(PathBuf::from(args.signatures))
        .expect("Error loading signatures.json");

    match args.check.is_dir() {
        true => match args.recursive {
            true => {
                let walker = WalkDir::new(&args.check).into_iter();
                for entry in walker.flatten() {
                    match entry.file_type().is_dir() {
                        true => continue,
                        false => match dirk::hank::analyze(entry.path(), &sigs) {
                            Ok(result) => {
                                print_result(&result, args.verbose);
                            },
                            Err(e) => {
                                if args.verbose {
                                    eprintln!("{} generated error :'{e}'", &entry.path().display());
                                }
                            },
                        },
                    }
                }
            }
            false => match dirk::hank::analyze(&args.check, &sigs) {
                Ok(result) => print_result(&result, args.verbose),
                Err(e) => eprintln!("{e}"),
            },
        },
        false => match dirk::hank::analyze(&args.check, &sigs) {
            Ok(_) => println!("{} OK", &args.check.display()),
            Err(e) => eprintln!("{e}"),
        },
    };
}
