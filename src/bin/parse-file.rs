use clap::Parser;
use dirk::phpxdebug;
use uuid::Uuid;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, value_parser)]
    file: String,
}
fn main() {
    let id = Uuid::new_v4();
    let args = Args::parse();
    let result = phpxdebug::parse_xtrace_file(id, args.file);
    match result {
        Ok(result) => {
            //result.print_tree();
            result.print_stats();
        }
        Err(e) => panic!("{e}"),
    }
}
