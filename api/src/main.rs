use clap::Parser;
use dirk_core::dirk_api::*;
use dirk_core::hank::*;
use dirk_core::models::dirk::DirkState;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, value_parser, default_value_t = SocketAddr::from(([127, 0, 0, 1], 3000)))]
    listen: SocketAddr,
    #[clap(short, long, value_parser, default_value_t = String::from("signatures.json"))]
    signatures: String,
}

#[tokio::main()]
async fn main() {
    let args = Args::parse();
    let db = get_db().await.expect("Unable to get a Database connection");
    let sigs = build_sigs_from_file(PathBuf::from(args.signatures))
        .expect("Failed to build signature objects");
    let app_state = Arc::new(DirkState { sigs, db });

    let addr: SocketAddr = args.listen;
    let scanner_app = build_router(app_state).expect("Failed to build router");
    axum::Server::bind(&addr)
        .serve(scanner_app.into_make_service())
        .await
        .expect("Unable to start our app");
}
