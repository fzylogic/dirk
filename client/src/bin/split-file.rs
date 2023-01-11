use clap::Parser;
use regex::RegexBuilder;

use std::fs::read_to_string;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, value_parser)]
    file: PathBuf,
}

fn write_parts(dir: &PathBuf, pieces: Vec<&str>) -> std::io::Result<String> {
    std::fs::create_dir(dir)?;
    let mut number: u32 = 1;
    for piece in pieces {
        std::fs::write(
            Path::new(dir).join(vec!["part", &number.to_string(), ".php"].join("")),
            piece,
        )?;
        number += 1;
    }
    Ok("done".to_string())
}

fn main() {
    let args = Args::parse();
    let mut _code_blocks: Vec<&str> = Vec::new();
    let re = RegexBuilder::new(r"(<\?php)")
        .case_insensitive(true)
        .dot_matches_new_line(true)
        .build()
        .unwrap();
    let data = read_to_string(&args.file).expect("Unable to read file");
    for caps in re.captures_iter(&data) {
        println!("{}", &caps["code"].len());
        //        println!("{:?}", &caps["code"]);
    }
    /*    let mut code_blocks: Vec<&str>;
    for block in data.split("<?php") {
        if !block.is_empty() {
            code_blocks.push(&block);
        }
    }

    for block in code_blocks {
        println!("{}", block.len());
    }*/
    /*    println!("Found {} distinct PHP enclosures", code_blocks.len());
        if code_blocks.len() > 1 {
            let path: PathBuf = args.file;
            println!("{:?}", path.as_path());
            match std::fs::remove_file(&path) {
                Ok(_result) => {
                    println!("removed {:?}", &path);
                    write_parts(&path, code_blocks).expect("Failed writing our parts out");
                },
                Err(e) => eprintln!("Encountered error: {e}"),
            }
        }
    }*/
}
