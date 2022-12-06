use std::ffi::OsStr;
use std::path::Path;
use clap::Parser;
use dirk::phpxdebug;
use uuid::Uuid;
use walkdir::{DirEntry, WalkDir};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, value_parser)]
    file: Option<String>,
    #[clap(short, long, value_parser)]
    dir: Option<String>,
}

fn is_xdebug_outfile(entry: &DirEntry) -> bool {
    entry.file_name()
        .to_str()
        .map(|s| s.ends_with(".xt"))
        .unwrap_or(false)
}

fn main() {
    let id = Uuid::new_v4();
    let args = Args::parse();

    let mut files: Vec<&Path> = Vec::new();

    match args.dir {
        Some(dir) => {
            let walker = WalkDir::new(dir).into_iter();
            for entry in walker.filter_entry(|e| is_xdebug_outfile(e)) {
                files.push(entry.unwrap().path());
                println!("{}", entry.unwrap().path().display());
            }
        },
        None => todo!()
    }

    for file in files {
        let result = phpxdebug_parser::parse_xtrace_file(id, file.into());
        match result {
            Ok(result) => {
                //result.print_tree();
                phpxdebug::print_stats(result);
            }
            Err(e) => panic!("{e}"),
        }
    }
}
