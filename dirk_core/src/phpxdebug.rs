use std::collections::{HashMap, HashSet};
use std::str;

use itertools::Itertools;
use lazy_static::lazy_static;
use phpxdebug_parser;
use phpxdebug_parser::XtraceEntryRecord;
use regex;
use regex::Regex;
use serde::{Deserialize, Serialize};

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
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum Tests {
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
pub fn analyze(file_record: &phpxdebug_parser::XtraceFileRecord) -> HashSet<Tests> {
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
