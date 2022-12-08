use clap::Parser;
use regex::RegexBuilder;

use std::fs::read_to_string;
use std::path::{PathBuf};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, value_parser)]
    file: PathBuf,
}

fn main() {
    let args = Args::parse();
    let mut code_blocks: Vec<&str> = Vec::new();
    let re = RegexBuilder::new(r"(?P<code><\?php.*\?>)")
        .case_insensitive(true)
        .dot_matches_new_line(true)
        .ignore_whitespace(true)
        .build()
        .unwrap();
    let data = read_to_string(&args.file).expect("Unable to read file");
    for caps in re.captures_iter(&data) {
        for code in caps.iter() {
            match code {
                Some(code_block) => code_blocks.push(code_block.as_str()),
                None => continue,
            }
        }
    }
    println!("Found {} distinct PHP enclosures", code_blocks.len());
    if code_blocks.len() > 1 {
        //print path.
    }
}
