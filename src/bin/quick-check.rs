use clap::Parser;
use dirk::hank::build_sigs_from_file;
use std::path::PathBuf;
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, value_parser)]
    check: PathBuf,
    #[clap(short, long, value_parser)]
    recursive: bool,
}

fn main() {
    let args = Args::parse();
    let sigs = build_sigs_from_file(PathBuf::from("/Users/fzylogic/signatures.json"))
        .expect("Error loading signatures.json");

    match args.check.is_dir() {
        true => match args.recursive {
            true => {
                let walker = WalkDir::new(&args.check).into_iter();
                for entry in walker.flatten() {
                    match entry.file_type().is_dir() {
                        true => continue,
                        false => match dirk::hank::analyze(entry.path(), &sigs) {
                            Ok(_) => println!("{} OK", &entry.path().display()),
                            Err(e) => eprintln!("{e}"),
                        },
                    }
                }
            }
            false => match dirk::hank::analyze(&args.check, &sigs) {
                Ok(_) => println!("{} OK", &args.check.display()),
                Err(e) => eprintln!("{e}"),
            },
        },
        false => match dirk::hank::analyze(&args.check, &sigs) {
            Ok(_) => println!("{} OK", &args.check.display()),
            Err(e) => eprintln!("{e}"),
        },
    };
}
