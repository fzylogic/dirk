use clap::Parser;
use dirk::phpxdebug;
use lazy_static::lazy_static;
use phpxdebug_parser::XtraceFileRecord;
use std::path::Path;
use uuid::Uuid;
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, value_parser)]
    file: Option<String>,
    #[clap(short, long, value_parser)]
    dir: Option<String>,
    #[clap(short, long)]
    no_stats: bool,
    #[clap(short, long)]
    tree: bool,
}

fn is_xdebug_outfile(entry: &walkdir::DirEntry) -> bool {
    if entry.file_type().is_dir() {
        return true;
    }
    entry
        .file_name()
        .to_str()
        .map(|s| s.ends_with(".xt"))
        .unwrap_or(false)
}

fn show_results(result: &XtraceFileRecord) {
    if !ARGS.no_stats {
        phpxdebug::print_stats(result);
    }
    if ARGS.tree {
        phpxdebug::print_tree(result);
    }
}

lazy_static! {
    static ref ARGS: Args = Args::parse();
}

fn main() {
    let _id = Uuid::new_v4();
    let args = Args::parse();

    match args.dir {
        Some(dir) => {
            let walker = WalkDir::new(dir).into_iter();
            for entry in walker.filter_entry(is_xdebug_outfile).flatten() {
                if entry.file_type().is_dir() {
                    continue;
                }
                match phpxdebug_parser::parse_xtrace_file(entry.path()) {
                    Ok(result) => {
                        show_results(&result);
                    }
                    Err(e) => eprintln!("Failed to process {} ({e})", entry.path().display()),
                }
            }
        }
        None => {
            let file = args.file.expect("No --dir or --file passed");
            match phpxdebug_parser::parse_xtrace_file(Path::new(file.as_str())) {
                Ok(result) => {
                    show_results(&result);
                }
                Err(e) => eprintln!("Failed to process {} ({e})", file),
            }
        }
    }
}
