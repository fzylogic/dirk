pub mod entities;

pub mod phpxdebug {
    use std::collections::{HashMap, HashSet};

    use itertools::Itertools;
    use lazy_static::lazy_static;
    use phpxdebug_parser;
    use phpxdebug_parser::XtraceEntryRecord;
    use regex;
    use regex::Regex;
    use std::str;

    fn is_within_eval(record: &XtraceEntryRecord) -> bool {
        record.file_name.contains(r"eval()'d code")
    }

    lazy_static! {
        static ref FISHY_FN_RE: regex::Regex = Regex::new(r"^[Oo]+$").unwrap();
    }

    fn fishy_fn_name(fn_name: &str) -> bool {
        FISHY_FN_RE.is_match(fn_name)
    }

    fn bad_fn_name(fn_name: &str) -> bool {
        ("curl_exec").contains(fn_name)
    }

    /*   struct FnScore {
        func_name: &'static str,
        adj_when_before: Option<fn() -> i32>,
        adj_when_after: Option<fn() -> i32>,
        only_when_before: Option<fn() -> bool>,
        only_when_after: Option<fn() -> bool>,
    }*/

    trait XtraceRecord {
        fn new(line: &str) -> Self;
    }
    trait XtraceFn {}

    #[allow(unused)]
    #[derive(Clone, Debug, Eq, Hash, PartialEq)]
    enum Tests {
        ErrorReportingDisabled,
        EvalPct(u8),
        Injected,
        KnownBadFnName(String),
        NetworkCallout,
        Obfuscated,
        OrdChrAlternation(u32),
        SingleLineOverload,
        SuspiciousFunction,
        UserProvidedEval,
    }

    pub fn print_tree(record: &phpxdebug_parser::XtraceFileRecord) {
        for record in record.fn_records.iter() {
            if let Some(entry_record) = &record.entry_record {
                let prefix = "  ".repeat(entry_record.level.try_into().unwrap());
                println!(
                    "{prefix}{}({:?}) ({}) ({})",
                    &entry_record.fn_name,
                    &entry_record.fn_type,
                    &entry_record.file_name,
                    &entry_record.inc_file_name
                );
            }
        }
    }

    pub fn print_stats(record: &phpxdebug_parser::XtraceFileRecord) {
        let mut num_fn_calls: u32 = 0;
        for entry in record.fn_records.iter() {
            if let Some(entry_record) = &entry.entry_record {
                num_fn_calls = std::cmp::max(num_fn_calls, entry_record.fn_num);
            }
        }
        let triggered_tests = analyze(record);
        if !triggered_tests.is_empty() {
            println!("{:?}:", &record.filename);
            println!("  Total function calls: {num_fn_calls}");
            println!("  {:?}", triggered_tests);
        }
    }

    pub fn print_timings(record: &phpxdebug_parser::XtraceFileRecord) {
        let mut fn_counts: HashMap<String, u64> = HashMap::new();
        let mut fn_timings: HashMap<String, f64> = HashMap::new();
        for entry in record.fn_records.iter() {
            if let Some(entry_record) = &entry.entry_record {
                if let Some(exit_record) = &entry.exit_record {
                    let duration = exit_record.time_idx - entry_record.time_idx;
                    fn_counts
                        .entry(entry_record.fn_name.to_string())
                        .and_modify(|counter| *counter += 1)
                        .or_insert(1);
                    fn_timings
                        .entry(entry_record.fn_name.to_string())
                        .and_modify(|counter| *counter += duration)
                        .or_insert(duration);
                }
            }
        }
        for fn_info in fn_timings
            .into_iter()
            .sorted_by(|a, b| PartialOrd::partial_cmp(&b.1, &a.1).unwrap())
        {
            println!(
                "Fn: {} Spent {}s across {} calls",
                fn_info.0,
                fn_info.1,
                fn_counts.get(&fn_info.0).unwrap_or(&0)
            );
        }
    }
    /// Length of chr()/ord() alternating sequences
    fn analyze(file_record: &phpxdebug_parser::XtraceFileRecord) -> HashSet<Tests> {
        let mut last: Option<&str> = None;
        let mut ordchr_count: u32 = 0;
        let mut fn_count: u32 = 0;
        let mut within_eval: u32 = 0;
        let mut counts: Vec<u32> = Vec::new();
        let fns = Vec::from(["ord", "chr"]);
        let mut tests_triggered: HashSet<Tests> = HashSet::new();
        for record in file_record.fn_records.iter() {
            //TODO this should probably be .map()
            if let Some(entry_record) = &record.entry_record {
                fn_count += 1;
                if fns.contains(&entry_record.fn_name.as_str()) {
                    match last {
                        Some(this_last) => {
                            if this_last != entry_record.fn_name {
                                ordchr_count += 1;
                                last = Some(entry_record.fn_name.as_str());
                            }
                        }
                        None => {
                            last = Some(entry_record.fn_name.as_str());
                            ordchr_count = 1;
                        }
                    }
                } else {
                    last = None;
                    if ordchr_count > 0 {
                        counts.push(ordchr_count);
                        ordchr_count = 0;
                    }
                }
                if fishy_fn_name(&entry_record.fn_name) {
                    tests_triggered.insert(Tests::KnownBadFnName(entry_record.fn_name.to_string()));
                }
                if bad_fn_name(&entry_record.fn_name) {
                    tests_triggered.insert(Tests::KnownBadFnName(entry_record.fn_name.to_string()));
                }
                if is_within_eval(entry_record) {
                    within_eval += 1;
                }
                if entry_record.fn_name == "error_reporting" && entry_record.args[0] == *"0" {
                    tests_triggered.insert(Tests::ErrorReportingDisabled);
                }
            }
        }
        let ordchr_count = counts.iter().max().unwrap_or(&0).to_owned();
        if ordchr_count > 1 {
            tests_triggered.insert(Tests::OrdChrAlternation(
                counts.iter().max().unwrap_or(&0).to_owned(),
            ));
        }
        if within_eval >= 1 {
            let eval_pct: u8 = ((within_eval as f32 / fn_count as f32) * 100.0) as u8;
            tests_triggered.insert(Tests::EvalPct(eval_pct));
        }
        tests_triggered
    }
}

pub mod hank {
    use crate::dirk_api;
    use base64;
    use serde::{de, Deserialize, Serialize};
    use serde_json;
    use serde_json::Value;
    use std::fmt;
    use std::fs::{read_to_string, File};
    use std::io::prelude::*;
    use std::io::BufReader;
    use std::path::{Path, PathBuf};

    #[derive(Clone, Copy, Debug, Deserialize, Serialize)]
    #[allow(non_camel_case_types)]
    pub enum Action {
        clean,
        disable,
        ignore,
    }
    #[derive(Clone, Debug, Deserialize, Serialize)]
    #[allow(non_camel_case_types)]
    pub enum Priority {
        high,
        medium,
    }
    #[derive(Clone, Debug, Deserialize, Serialize)]
    #[allow(non_camel_case_types)]
    pub enum Severity {
        red,
        yellow,
    }
    #[derive(Clone, Debug, Deserialize, Serialize)]
    #[allow(non_camel_case_types)]
    pub enum Target {
        Default,
        DEFAULT_TARGET,
        HTACCESS,
        HTML,
        INTERPRETED,
        JAVASCRIPT,
        PERL,
        PHP,
        PYTHON,
        SHELL,
    }
    #[derive(Clone, Debug, Deserialize)]
    pub enum Type {
        Backdoor,
    }

    #[derive(Clone, Debug, Deserialize, Serialize)]
    pub struct Signature {
        pub action: Action,
        pub comment: String,
        pub date: u64,
        pub filenames: Vec<String>,
        #[serde(deserialize_with = "deserialize_bool")]
        pub flat_string: bool,
        pub id: String,
        pub priority: Priority,
        pub severity: Severity,
        pub signature: String,
        pub submitter: String,
        pub target: Target,
    }

    pub type ResultStatus = dirk_api::DirkResultClass;

    impl fmt::Display for ResultStatus {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                ResultStatus::OK => write!(f, "OK"),
                ResultStatus::Bad => write!(f, "BAD"),
                ResultStatus::Inconclusive => write!(f, "Inconclusive"),
            }
        }
    }
    #[derive(Debug, Serialize)]
    pub struct ScanResult {
        pub filename: PathBuf,
        pub signature: Option<Signature>,
        pub status: ResultStatus,
    }

    fn deserialize_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        Ok(match Value::deserialize(deserializer)? {
            Value::Bool(b) => b,
            Value::String(s) => s == "yes",
            Value::Number(num) => {
                num.as_i64()
                    .ok_or_else(|| de::Error::custom("Invalid number; cannot convert to bool"))?
                    != 0
            }
            Value::Null => false,
            _ => return Err(de::Error::custom("Wrong type, expected boolean")),
        })
    }
    pub fn build_sigs_from_file(filename: PathBuf) -> Result<Vec<Signature>, std::io::Error> {
        let file = File::open(filename)?;
        let mut results = Vec::new();
        let mut buf = String::new();
        let mut reader = BufReader::new(file);
        loop {
            let len = reader.read_line(&mut buf)?;
            if len == 0 {
                break;
            }
            let sig: Signature =
                serde_json::from_str(&buf).expect("Unable to parse line into a Signature");
            results.push(sig);
            buf.clear();
        }
        Ok(results)
    }
    //TODO This should be a Signature method
    fn decode_sig_to_pattern(sig: &Signature) -> String {
        if sig.signature.contains('\n') {
            let mut temp = String::new();
            for part in sig.signature.split('\n') {
                let decoded_part = base64::decode(part).expect("Unable to decode signature");
                let decoded_sig = std::str::from_utf8(&decoded_part).unwrap();
                if temp.is_empty() {
                    temp = decoded_sig.to_string();
                } else {
                    temp = format!("{}\n{}", &temp, &decoded_sig);
                }
            }
            temp
        } else {
            return std::str::from_utf8(
                &base64::decode(&sig.signature).expect("Unable to decode signature"),
            )
            .unwrap()
            .to_string();
        }
    }

    pub fn analyze_file(
        filename: &Path,
        sigs: &Vec<Signature>,
    ) -> Result<ScanResult, std::io::Error> {
        let file_data = read_to_string(filename)?;
        analyze_file_data(&file_data, filename, sigs)
    }

    pub fn analyze_file_data(
        file_data: &str,
        filename: &Path,
        sigs: &Vec<Signature>,
    ) -> Result<ScanResult, std::io::Error> {
        for sig in sigs {
            let pattern = decode_sig_to_pattern(sig);
            //println!("Testing pattern ({pattern})");
            if file_data.contains(&pattern) {
                return Ok(ScanResult {
                    filename: filename.to_owned(),
                    status: ResultStatus::Bad,
                    signature: Some(sig.to_owned()),
                });
            }
        }
        Ok(ScanResult {
            filename: filename.to_owned(),
            status: ResultStatus::OK,
            signature: None,
        })
    }
}

pub mod dirk_api {
    use axum::error_handling::HandleErrorLayer;
    use axum::extract::{Path, State};
    use axum::http::Uri;
    use axum::response::IntoResponse;
    use axum::routing::get;
    use axum::{
        extract::DefaultBodyLimit, http::StatusCode, routing::post, BoxError, Json, Router,
    };
    use serde::{Deserialize, Serialize};
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    use clap::ValueEnum;
    use sea_orm::entity::prelude::*;
    use std::collections::HashMap;
    use std::fmt;
    use std::fmt::Error;

    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;

    use sea_orm::ActiveValue::Set;
    use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, DbErr, Statement};
    use serde_json::{json, Value};
    use tower::ServiceBuilder;
    use tower_http::trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer};
    use tower_http::LatencyUnit;
    use tracing::Level;

    use uuid::Uuid;

    use crate::entities::prelude::*;
    use crate::entities::sea_orm_active_enums::*;
    use crate::entities::*;
    use crate::hank::*;

    pub fn build_router(app_state: Arc<DirkState>) -> Router {
        tracing_subscriber::registry()
            .with(tracing_subscriber::EnvFilter::new(
                std::env::var("RUST_LOG").unwrap_or_else(|_| "tower_http=debug".into()),
            ))
            .with(tracing_subscriber::fmt::layer())
            .init();

        Router::new()
            .route("/health-check", get(health_check))
            .route("/scanner/quick", post(quick_scan))
            .route("/scanner/full", post(full_scan))
            .route("/scanner/dynamic", post(dynamic_scan_api))
            .route("/files/update", post(update_file_api))
            .route("/files/list", get(list_known_files))
            .route("/files/get/:sha256sum", get(get_file_status_api))
            .layer(DefaultBodyLimit::disable())
            // Add middleware to all routes
            .layer(
                ServiceBuilder::new()
                    .layer(HandleErrorLayer::new(|error: BoxError| async move {
                        if error.is::<tower::timeout::error::Elapsed>() {
                            Ok(StatusCode::REQUEST_TIMEOUT)
                        } else {
                            Err((
                                StatusCode::INTERNAL_SERVER_ERROR,
                                format!("Unhandled internal error: {}", error),
                            ))
                        }
                    }))
                    .timeout(Duration::from_secs(120))
                    .into_inner(),
            )
            .layer(
                TraceLayer::new_for_http()
                    .make_span_with(DefaultMakeSpan::new().include_headers(true))
                    .on_request(DefaultOnRequest::new().level(Level::INFO))
                    .on_response(
                        DefaultOnResponse::new()
                            .level(Level::INFO)
                            .latency_unit(LatencyUnit::Micros),
                    ),
            )
            .with_state(app_state)
    }
    const DATABASE_URL: &str = "mysql://dirk:ahghei4phahk5Ooc@localhost:3306/dirk";

    ///Full scan inspects the list of known sha256 digests as well as scanning file content
    async fn full_scan(
        State(state): State<Arc<DirkState>>,
        Json(bulk_payload): Json<ScanBulkRequest>,
    ) -> impl IntoResponse {
        let mut results: Vec<ScanResult> = Vec::new();
        let code = StatusCode::OK;
        for payload in bulk_payload.requests {
            let file_path = payload.file_name;
            if !payload.skip_cache {
                if let Some(file) = fetch_status(&state.db, payload.sha256sum.clone()).await {
                    let result = ScanResult {
                        file_names: Vec::from([file_path]),
                        sha256sum: file.sha256sum,
                        result: match file.file_status {
                            FileStatus::Good => DirkResultClass::OK,
                            FileStatus::Bad => DirkResultClass::Bad,
                            FileStatus::Whitelisted => DirkResultClass::OK,
                            FileStatus::Blacklisted => DirkResultClass::Bad,
                        },
                        reason: DirkReason::Cached,
                        cache_detail: None,
                        signature: None,
                    };
                    results.push(result);
                    continue;
                }
            }
            // We only reach this point if `skip_cache` wasn't set on the request AND
            // if the file wasn't able to be fetched from our cache.
            let result = match analyze_file_data(
                &payload.file_contents.unwrap_or_default(),
                &file_path,
                &state.sigs,
            ) {
                Ok(scanresult) => ScanResult {
                    file_names: Vec::from([file_path]),
                    sha256sum: payload.sha256sum.clone(),
                    result: scanresult.status,
                    reason: DirkReason::LegacyRule,
                    cache_detail: None,
                    signature: scanresult.signature,
                },
                Err(e) => {
                    eprintln!("Error encountered: {e}");
                    ScanResult {
                        file_names: Vec::from([file_path]),
                        sha256sum: payload.sha256sum.clone(),
                        result: DirkResultClass::Inconclusive,
                        reason: DirkReason::InternalError,
                        cache_detail: None,
                        signature: None,
                    }
                }
            };
            if let DirkResultClass::Bad = result.result {
                let csum = result.sha256sum.clone();
                let file = FileUpdateRequest {
                    checksum: csum,
                    file_status: FileStatus::Bad,
                };
                let _res = create_or_update_file(file, &state.db).await;
            }
            results.push(result);
        }
        let id = Uuid::new_v4();
        let bulk_result = ScanBulkResult { id, results };
        (code, Json(bulk_result)).into_response()
    }

    ///Quick scan that only looks up sha256 digests against the database
    async fn quick_scan(
        State(state): State<Arc<DirkState>>,
        Json(bulk_payload): Json<ScanBulkRequest>,
    ) -> impl IntoResponse {
        let code = StatusCode::OK;
        let db = &state.db;

        let mut sums: Vec<String> = Vec::new();
        let mut sum_map: HashMap<String, Vec<PathBuf>> = HashMap::new();

        for req in bulk_payload.requests {
            let file_name = req.file_name.clone();
            sum_map
                .entry(req.sha256sum.to_string())
                .and_modify(|this_map| this_map.push(req.file_name))
                .or_insert_with(|| Vec::from([file_name]));
            sums.push(req.sha256sum);
        }

        let files: Vec<files::Model> = Files::find()
            .filter(files::Column::Sha256sum.is_in(sums))
            .all(db)
            .await
            .unwrap();

        let results = files
            .into_iter()
            .map(|file| {
                let sha256sum = file.sha256sum.clone();
                let status = file.file_status;
                let class = match status {
                    FileStatus::Bad | FileStatus::Blacklisted => DirkResultClass::Bad,
                    FileStatus::Good | FileStatus::Whitelisted => DirkResultClass::OK,
                };
                ScanResult {
                    file_names: sum_map[&sha256sum].clone(),
                    cache_detail: Some(status),
                    reason: DirkReason::Cached,
                    signature: None,
                    result: class,
                    sha256sum: file.sha256sum,
                }
            })
            .collect();
        let id = Uuid::new_v4();
        let bulk_result = ScanBulkResult { id, results };
        (code, Json(bulk_result)).into_response()
    }

    async fn health_check(State(state): State<Arc<DirkState>>) -> impl IntoResponse {
        let db = &state.db;
        let stmt =
            Statement::from_string(DbBackend::MySql, "select count(*) from files".to_owned());
        if let Ok(result) = db.query_one(stmt).await {
            (
                StatusCode::OK,
                format!("Hi! All's good here. {:#?}", result.unwrap()),
            )
                .into_response()
        } else {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Database connection failed",
            )
                .into_response()
        }
    }

    pub async fn get_db() -> Result<DatabaseConnection, DbErr> {
        Database::connect(DATABASE_URL).await
    }

    ///Dump a listing of all known files
    async fn list_known_files(State(state): State<Arc<DirkState>>) -> Json<Value> {
        let db = &state.db;
        let files: Vec<files::Model> = Files::find().all(db).await.unwrap();
        Json(json!(files))
    }

    ///Fetch a single File record from the database
    async fn fetch_status(db: &DatabaseConnection, csum: String) -> Option<files::Model> {
        Files::find()
            .filter(files::Column::Sha256sum.eq(csum))
            .one(db)
            .await
            .unwrap()
    }

    ///API to run a dynamic analysis on a set of files
    async fn dynamic_scan_api(
        State(state): State<Arc<DirkState>>,
        Path(_sha256sum): Path<String>,
    ) -> Json<Value> {
        let _db = &state.db;
        Json(Value::String("asdf".to_string()))
    }

    ///API to retrieve a single file record
    async fn get_file_status_api(
        State(state): State<Arc<DirkState>>,
        Path(sha256sum): Path<String>,
    ) -> Json<Value> {
        let db = &state.db;
        println!("Fetching file status for {}", &sha256sum);
        Json(json!(fetch_status(db, sha256sum).await))
    }

    ///Update a file record in the database
    async fn update_file(
        rec: files::Model,
        req: FileUpdateRequest,
        db: &DatabaseConnection,
    ) -> Result<(), Error> {
        let mut rec: files::ActiveModel = rec.into();
        rec.last_updated = Set(DateTime::default());
        rec.file_status = Set(req.file_status);
        rec.update(db).await.unwrap();
        Ok(())
    }

    ///Create a new fie record in the database
    async fn create_file(req: FileUpdateRequest, db: &DatabaseConnection) -> Result<(), Error> {
        let file = files::ActiveModel {
            sha256sum: Set(req.checksum),
            file_status: Set(req.file_status),
            ..Default::default()
        };
        println!("Creating new file");
        let _file = file.insert(db).await.unwrap();
        Ok(())
    }

    ///Wrapper to create or update a file record
    async fn create_or_update_file(
        file: FileUpdateRequest,
        db: &DatabaseConnection,
    ) -> impl IntoResponse {
        let csum = file.checksum.clone();
        let file_record: Option<files::Model> = Files::find()
            .filter(files::Column::Sha256sum.eq(csum))
            .one(db)
            .await
            .unwrap();
        match file_record {
            Some(rec) => update_file(rec, file, db).await.unwrap(),
            None => create_file(file, db).await.unwrap(),
        }
    }

    ///API endpoint to update a file record
    async fn update_file_api(
        State(state): State<Arc<DirkState>>,
        Json(file): Json<FileUpdateRequest>,
    ) -> impl IntoResponse {
        let db = &state.db;
        create_or_update_file(file, db).await
    }

    /// The Type of result we've received about a file
    #[derive(Copy, Clone, Debug, Deserialize, Serialize)]
    pub enum DirkResultClass {
        Bad,
        Inconclusive,
        OK,
    }

    /// The reasoning behind the result we received
    #[derive(Copy, Clone, Debug, Deserialize, Serialize)]
    pub enum DirkReason {
        Cached,
        InternalError,
        LegacyRule,
        None,
    }

    /// Request to update a file record
    #[derive(Debug, Deserialize, Serialize)]
    pub struct FileUpdateRequest {
        pub checksum: String,
        pub file_status: FileStatus,
    }

    impl fmt::Display for DirkReason {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                DirkReason::Cached => write!(f, "Cached SHA256SUM"),
                DirkReason::InternalError => write!(f, "Internal Error encountered"),
                DirkReason::None => write!(f, "No reason; something must have gone wrong"),
                DirkReason::LegacyRule => write!(f, "Legacy Hank rule was triggered"),
            }
        }
    }

    /// The typed of scan requests currently supported
    #[derive(Clone, Debug, ValueEnum, Deserialize, Serialize)]
    pub enum ScanType {
        FindUnknown,
        Full,
        Quick,
    }

    impl ScanType {
        pub fn url(&self, urlbase: Uri) -> String {
            match self {
                ScanType::Full => format!("{}{}", urlbase, "scanner/full"),
                ScanType::Quick => format!("{}{}", urlbase, "scanner/quick"),
                _ => "".to_string(),
            }
        }
    }

    #[derive(Clone, Debug, Deserialize, Serialize)]
    pub struct ScanRequest {
        pub sha256sum: String,
        pub kind: ScanType,
        pub file_name: PathBuf,
        pub file_contents: Option<String>,
        pub skip_cache: bool,
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct ScanBulkRequest {
        pub requests: Vec<ScanRequest>,
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct ScanResult {
        pub file_names: Vec<PathBuf>,
        pub sha256sum: String,
        pub result: DirkResultClass,
        pub reason: DirkReason,
        pub cache_detail: Option<FileStatus>,
        pub signature: Option<Signature>,
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct ScanBulkResult {
        pub id: Uuid,
        pub results: Vec<ScanResult>,
    }

    //#[derive(Clone)]
    pub struct DirkState {
        pub sigs: Vec<Signature>,
        pub db: DatabaseConnection,
    }
}

/*pub mod analysis_api {
    pub async fn api() {

    }
}*/

pub mod container {
    /*    use std::process::Command;
    pub fn docker_examine() {
        let docker = Command::new("docker")
            .arg("--rm")
            .arg("--network none")
            .arg("-u sandbox")
            .arg("-v -v ~/code:/usr/src/sandbox")
            .arg("-v -v ~/results:/usr/src/results")
            .arg("-w /usr/src/sandbox")
            .arg("dreamhost/php-8.0-xdebug:production")
            .arg("bash /usr/local/bin/check.sh");
    }*/
    /* Workflow is as follows:
     * Client uploads files via the API
     * Server then dumps the files into a tempdir
     * A container is spun up using our custom php/xdebug image w/ the tempdir mounted as a read-only volume
     * This container has no networking or other privileges
     * A second read-only volume is mounted, which contains a socket for communication back to the host.+
     * Once analysis is complete and the results have been reported back via the socket, the container is shut down
     */

    //use podman_api::Podman;

    use std::path::Path;
    #[allow(dead_code)]
    fn init(_path: &Path) {}

    pub fn examine(_path: &Path) {}
}

pub mod util {
    use sha256::digest;

    pub fn checksum(data: &String) -> String {
        digest(data.to_string())
    }
}
