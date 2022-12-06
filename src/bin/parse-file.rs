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
    let result = phpxdebug_parser::parse_xtrace_file(id, args.file);
    match result {
        Ok(result) => {
            //result.print_tree();
            phpxdebug::print_stats(result);
        }
        Err(e) => panic!("{e}"),
    }
}
