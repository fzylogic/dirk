use std::collections::HashMap;
use std::fmt::Error;

use axum::error_handling::HandleErrorLayer;
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{extract::DefaultBodyLimit, http::StatusCode, routing::post, BoxError, Json, Router};
use clap::Parser;
use sea_orm::entity::prelude::*;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use sea_orm::ActiveValue::Set;
use sea_orm::{Database, DatabaseConnection, DbErr};
use serde_json::{json, Value};
use tower::ServiceBuilder;
use tower_http::trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tower_http::LatencyUnit;
use tracing::Level;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use uuid::Uuid;

use dirk_core::dirk_api::*;
use dirk_core::entities::prelude::*;
use dirk_core::entities::sea_orm_active_enums::*;
use dirk_core::entities::*;
use dirk_core::hank::*;

#[derive(Parser, Debug)]
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
    let db = dirk_core::dirk_api::get_db().await.unwrap();
    let sigs = build_sigs_from_file(PathBuf::from(args.signatures)).unwrap();
    let app_state = Arc::new(DirkState { sigs, db });

    let addr: SocketAddr = args.listen;
    let scanner_app = build_router(app_state);
    axum::Server::bind(&addr)
        .serve(scanner_app.into_make_service())
        .await
        .unwrap();
}
