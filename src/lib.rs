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

    pub type ResultStatus = dirk_api::DirkResult;

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
    use serde::{Deserialize, Serialize};
    use std::fmt;

    use crate::hank::Signature;
    use std::path::PathBuf;
    use uuid::Uuid;

    #[derive(Copy, Clone, Debug, Deserialize, Serialize)]
    pub enum DirkResult {
        Bad,
        Inconclusive,
        OK,
    }

    #[derive(Copy, Clone, Debug, Deserialize, Serialize)]
    pub enum DirkReason {
        InternalError,
        LegacyRule,
        None,
    }

    impl fmt::Display for DirkReason {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                DirkReason::InternalError => write!(f, "Internal Error encountered"),
                DirkReason::None => write!(f, "No reason; something must have gone wrong"),
                DirkReason::LegacyRule => write!(f, "Legacy Hank rule was triggered"),
            }
        }
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct QuickScanRequest {
        pub file_name: PathBuf,
        pub file_contents: String,
        pub checksum: String,
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct QuickScanBulkRequest {
        pub requests: Vec<QuickScanRequest>,
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct QuickScanResult {
        pub file_name: PathBuf,
        pub result: DirkResult,
        pub reason: DirkReason,
        pub signature: Option<Signature>,
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct QuickScanBulkResult {
        pub id: Uuid,
        pub results: Vec<QuickScanResult>,
    }
/*    impl QuickScanBulkResult {
        fn combine_results(&mut self, new_results: &mut QuickScanBulkResult) {
            self.results.append(&mut new_results.results);
        }
    }*/

}
