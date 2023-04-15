use clap::Parser;
use dirk_core::dirk_api::*;

use dirk_core::models::dirk::DirkState;
use std::net::SocketAddr;

use std::sync::Arc;
use walkdir::WalkDir;
use yara::Compiler;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, value_parser, default_value_t = SocketAddr::from(([127, 0, 0, 1], 3000)))]
    listen: SocketAddr,
    #[clap(short, long, value_parser)]
    ruledir: String,
}

#[tokio::main()]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let db = get_db().await.expect("Unable to get a Database connection");
    let mut yc = Compiler::new().unwrap();
    for entry in WalkDir::new(args.ruledir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let tc = Compiler::new().unwrap();
        if let Ok(_tmp) = tc.add_rules_file(entry.path()) {
            yc = yc.add_rules_file(entry.path()).unwrap();
        }
    }
    let rules = yc.compile_rules()?;
    let app_state = Arc::new(DirkState { rules, db });

    let addr: SocketAddr = args.listen;
    let scanner_app = build_router(app_state).expect("Failed to build router");
    axum::Server::bind(&addr)
        .serve(scanner_app.into_make_service())
        .await
        .expect("Unable to start our app");
    Ok(())
}
