pub mod phpxdebug {
    use std::collections::HashSet;

    use std::str;

    use lazy_static::lazy_static;
    use phpxdebug_parser;
    use phpxdebug_parser::XtraceEntryRecord;
    use regex;
    use regex::Regex;

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
        //println!("Length of longest chr()/ord() alternating sequence: {}", self.chr_ord_alter());
        //println!("Utilized a known-fishy function name? {}", self.fishy_fn_name());
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
    use base64;
    use serde::{de, Deserialize, Serialize};
    use serde_json;
    use std::fs::{read_to_string, File};
    use std::io::prelude::*;
    use std::io::BufReader;
    use std::path::{Path, PathBuf};
    #[derive(Clone, Copy, Deserialize, Serialize)]
    #[allow(non_camel_case_types)]
    pub enum Action {
        clean,
        disable,
        ignore,
    }
    #[derive(Deserialize)]
    #[allow(non_camel_case_types)]
    pub enum Priority {
        high,
        medium,
    }
    #[derive(Deserialize)]
    #[allow(non_camel_case_types)]
    pub enum Severity {
        red,
        yellow,
    }
    #[derive(Deserialize)]
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
    #[derive(Deserialize)]
    #[allow(non_camel_case_types)]
    pub enum Type {
        Backdoor,
    }

    #[derive(Deserialize)]
    pub struct Signature {
        pub action: Action,
        pub comment: String,
        pub date: u64,
        pub filenames: Option<Vec<String>>,
        #[serde(deserialize_with = "deserialize_bool")]
        pub flat_string: bool,
        pub id: String,
        pub priority: Priority,
        pub severity: Severity,
        pub signature: String,
        pub submitter: String,
        pub target: Target,
    }

    #[derive(Serialize)]
    pub enum ResultStatus {
        OK,
        BAD,
    }
    #[derive(Serialize)]
    pub struct ScanResult {
        pub filename: PathBuf,
        pub status: ResultStatus,
        pub suggested_action: Action,
    }

    fn deserialize_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let s: u64 = de::Deserialize::deserialize(deserializer)?;

        match s {
            1 => Ok(true),
            0 => Ok(false),
            _ => Err(de::Error::unknown_variant(&s.to_string(), &["1", "0"])),
        }
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
    fn decode_sig_to_pattern(sig: &Signature) -> String {
        //println!("Processing signature {}: {}", &sig.id, &sig.signature);
        if sig.signature.contains("\n") {
            //println!("Sig {} contains a newline", sig.id);
            let mut temp = String::new();
            for part in sig.signature.split("\n") {
                let decoded_part = base64::decode(part).expect("Unable to decode signature");
                let decoded_sig = std::str::from_utf8(&decoded_part).unwrap();
                if temp.len() == 0 {
                    temp = decoded_sig.to_string();
                } else {
                    temp = format!("{}\n{}", &temp, &decoded_sig);
                }
            }
            return temp;
        } else {
            //println!("Sig {} does NOT contain a newline", sig.id);
            return std::str::from_utf8(
                &base64::decode(&sig.signature).expect("Unable to decode signature"),
            )
            .unwrap()
            .to_string();
        }
    }
    pub fn analyze(filename: &Path, sigs: &Vec<Signature>) -> Result<ScanResult, std::io::Error> {
        let file_data = read_to_string(filename)?;
        let mut status = ResultStatus::OK;
        let mut suggested_action = Action::ignore;
        for sig in sigs {
            let pattern = decode_sig_to_pattern(&sig);
            //println!("Testing pattern ({pattern})");
            if file_data.contains(&pattern) {
                status = ResultStatus::BAD;
                suggested_action = sig.action;
                break;
            }
        }
        Ok(ScanResult {
            filename: filename.to_owned(),
            status,
            suggested_action,
        })
    }
}
