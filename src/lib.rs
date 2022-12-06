pub mod phpxdebug {
    use std::collections::{HashMap, HashSet};
    //use std::fmt;
    use std::fs::File;
    use std::io::prelude::*;
    use std::io::BufReader;
    
    use std::str;

    use lazy_static::lazy_static;
    use regex;
    use regex::{Regex, RegexSet};

    static SUPPORTED_FILE_FORMATS: &[&str] = &["4"];

    lazy_static! {
        static ref RE_SET: regex::RegexSet = RegexSet::new([
            LineRegex::Version.regex_str(),
            LineRegex::Format.regex_str(),
            LineRegex::Start.regex_str(),
            LineRegex::FunctionEntry.regex_str(),
            LineRegex::FunctionExit.regex_str(),
            LineRegex::Penultimate.regex_str(),
            LineRegex::End.regex_str(),
        ])
        .unwrap();
    }
    lazy_static! {
        static ref FISHY_FN_RE: regex::Regex = Regex::new(r"^[Oo]+$").unwrap();
    }

    fn fishy_fn_name(fn_name: &String) -> bool {
        FISHY_FN_RE.is_match(fn_name)
    }
    fn process_line(
        run: &mut XtraceFileRecord,
        entry_cache: &mut HashMap<u32, XtraceEntryRecord>,
        line: &String,
    ) {
        let matches: Vec<_> = RE_SET.matches(line.as_str()).into_iter().collect();
        if matches.is_empty() {
            eprintln!("No matches for line: {line}");
            return;
        }
        let idx = matches.first().unwrap();
        match idx {
            0 => run.version = Some(XtraceVersionRecord::new(line)),
            1 => run.format = Some(XtraceFmtRecord::new(line)),
            2 => run.start = Some(XtraceStartTimeRecord::new(line)),
            3 => {
                let record = XtraceEntryRecord::new(line);
                entry_cache.insert(record.fn_num, record);
            }
            4 => {
                let exit_record = XtraceExitRecord::new(line);
                if let Some(entry_record) = entry_cache.get(&exit_record.fn_num) {
                    let fn_record = XtraceFnRecord {
                        fn_num: exit_record.fn_num,
                        entry_record: Some(entry_record.to_owned()),
                        exit_record: Some(exit_record),
                    };
                    run.add_fn_record(fn_record);
                }
            }
            5 => {}
            6 => {}
            _ => todo!(),
        };
    }
    pub fn parse_xtrace_file(
        id: uuid::Uuid,
        file: String,
    ) -> Result<XtraceFileRecord, std::io::Error> {
        let xtrace_file = File::open(file)?;
        let mut reader = BufReader::new(xtrace_file);
        //let mut line = String::new();
        let mut line: Vec<u8> = Vec::new();
        let mut run = XtraceFileRecord {
            id,
            format: None,
            start: None,
            version: None,
            fn_records: Vec::new(),
        };
        let mut entry_cache: HashMap<u32, XtraceEntryRecord> = HashMap::new();
        let mut line_number: u32 = 1;
        loop {
            //let result = reader.read_line(&mut line);
            let result = reader.read_until(0xA, &mut line);
            match result {
                Ok(size) => {
                    if size == 0 {
                        return Ok(run);
                    }
                    //println!("Processing line {line_number}: {line}");
                    process_line(
                        &mut run,
                        &mut entry_cache,
                        &String::from_utf8_lossy(line.as_slice()).to_string(),
                    );
                }
                Err(e) => {
                    eprintln!("Error reading line #{line_number}: {e}");
                    continue;
                }
            }
            line_number += 1;
            line.clear();
        }
    }

    struct FnScore {
        func_name: &'static str,
        adj_when_before: Option<fn() -> i32>,
        adj_when_after: Option<fn() -> i32>,
        only_when_before: Option<fn() -> bool>,
        only_when_after: Option<fn() -> bool>,
    }

    #[derive(Clone, Debug)]
    enum RecType {
        Entry,
        Exit,
        Format,
        StartTime,
        Version,
    }

    trait XtraceRecord {
        fn new(line: &str) -> Self;
    }
    trait XtraceFn {}

    #[allow(unused)]
    #[derive(Clone, Debug)]
    pub struct XtraceFileRecord {
        id: uuid::Uuid,
        start: Option<XtraceStartTimeRecord>,
        format: Option<XtraceFmtRecord>,
        version: Option<XtraceVersionRecord>,
        fn_records: Vec<XtraceFnRecord>,
    }

    #[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
    enum Tests {
        ErrorReportingDisabled,
        EvalPct(u8),
        Injected,
        KnownBadFnName,
        NetworkCallout,
        Obfuscated,
        OrdChrAlternation(u32),
        SingleLineOverload,
        UserProvidedEval,
    }

    impl XtraceFileRecord {
        fn add_fn_record(&mut self, func: XtraceFnRecord) {
            self.fn_records.push(func);
        }
        pub fn score(&self) -> u32 {
            let mut score = 0;
            for record in self.fn_records.iter() {
                score += record.score();
            }
            score
        }

        pub fn print_tree(&self) {
            for record in self.fn_records.iter() {
                if let Some(entry_record) = &record.entry_record {
                    let prefix = "  ".repeat(entry_record.level.try_into().unwrap());
                    println!(
                        "{prefix}{}({}) ({}) ({})",
                        &entry_record.fn_name,
                        &entry_record.fn_type,
                        &entry_record.file_name,
                        &entry_record.inc_file_name
                    );
                }
            }
        }
        // Want to look at the following
        // What % of fn calls are from within eval() blocks
        // Any network fns?
        // Signs of obfuscation? (calls to ord(), etc)
        // eval from user-provided data
        // fn name matches /^[oO]{3,}/
        // disabling error reporting?
        // single lines running large numbers of functions?

        pub fn print_stats(&self) {
            let mut num_fn_calls: u32 = 0;
            for record in self.fn_records.iter() {
                if let Some(entry_record) = &record.entry_record {
                    num_fn_calls = std::cmp::max(num_fn_calls, entry_record.fn_num);
                }
            }
            println!("Total function calls: {num_fn_calls}");
            let triggered_tests = self.analyze();
            println!("{:?}", triggered_tests);
            //println!("Length of longest chr()/ord() alternating sequence: {}", self.chr_ord_alter());
            //println!("Utilized a known-fishy function name? {}", self.fishy_fn_name());
        }
        /// Length of chr()/ord() alternating sequences
        fn analyze(&self) -> HashSet<Tests> {
            let mut last: Option<&str> = None;
            let mut ordchr_count: u32 = 0;
            let mut fn_count: u32 = 0;
            let mut within_eval: u32 = 0;
            let mut counts: Vec<u32> = Vec::new();
            let fns = Vec::from(["ord", "chr"]);
            let mut tests_triggered: HashSet<Tests> = HashSet::new();
            for record in self.fn_records.iter() {
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
                        tests_triggered.insert(Tests::KnownBadFnName);
                    }
                    if entry_record.within_eval() {
                        within_eval += 1;
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
    impl XtraceFnRecord {
        fn score(&self) -> u32 {
            1
        }
    }
    #[allow(unused)]
    #[derive(Clone, Debug)]
    pub struct XtraceFnRecord {
        fn_num: u32,
        entry_record: Option<XtraceEntryRecord>,
        exit_record: Option<XtraceExitRecord>,
        //return_record: Option<XtraceReturnRecord>,
    }

    impl XtraceRecord for XtraceVersionRecord {
        fn new(line: &str) -> Self {
            let re = Regex::new(LineRegex::Version.regex_str()).unwrap();
            let cap = re.captures(line).unwrap();
            let version = cap
                .name("version")
                .expect("version number not found")
                .as_str()
                .to_owned();
            XtraceVersionRecord {
                version,
                rec_type: RecType::Version,
            }
        }
    }
    #[allow(unused)]
    #[derive(Clone, Debug)]
    pub struct XtraceVersionRecord {
        version: String,
        rec_type: RecType,
    }

    impl XtraceRecord for XtraceStartTimeRecord {
        fn new(line: &str) -> Self {
            let re = Regex::new(LineRegex::Start.regex_str()).unwrap();
            let _cap = re.captures(line).ok_or("oops").unwrap();
            XtraceStartTimeRecord {
                start_time: String::from("Sat Dec  3 18:01:30 PST 2022"),
                rec_type: RecType::StartTime,
            }
        }
    }
    #[allow(unused)]
    #[derive(Clone, Debug)]
    pub struct XtraceStartTimeRecord {
        start_time: String,
        rec_type: RecType,
    }

    impl XtraceRecord for XtraceFmtRecord {
        fn new(line: &str) -> Self {
            let re = Regex::new(LineRegex::Format.regex_str()).unwrap();
            let cap = re.captures(line).ok_or("oops").unwrap();
            let format = cap
                .name("format")
                .expect("version number not found")
                .as_str();
            if SUPPORTED_FILE_FORMATS.contains(&format) {
                XtraceFmtRecord {
                    format: format
                        .parse::<u32>()
                        .expect("Unable to parse format number into an integer"),
                    rec_type: RecType::Format,
                }
            } else {
                panic!("Unsupported version: {}", format);
            }
        }
    }
    #[allow(unused)]
    #[derive(Clone, Debug)]
    pub struct XtraceFmtRecord {
        format: u32,
        rec_type: RecType,
    }

    /*    enum FnType {
        Internal,
        User,
    }*/

    impl XtraceEntryRecord {
        fn within_eval(&self) -> bool {
            self.file_name.contains(r"eval()'d code")
        }
    }

    impl XtraceFn for XtraceEntryRecord {}
    impl XtraceRecord for XtraceEntryRecord {
        fn new(line: &str) -> Self {
            let re = Regex::new(LineRegex::FunctionEntry.regex_str()).unwrap();
            let cap = re.captures(line).ok_or("oops").unwrap();
            return XtraceEntryRecord {
                rec_type: RecType::Entry,
                level: cap.name("level").unwrap().as_str().parse::<u32>().unwrap(),
                fn_num: cap.name("fn_num").unwrap().as_str().parse::<u32>().unwrap(),
                time_idx: cap
                    .name("time_idx")
                    .unwrap()
                    .as_str()
                    .parse::<f64>()
                    .unwrap(),
                mem_usage: cap
                    .name("mem_usage")
                    .unwrap()
                    .as_str()
                    .parse::<u32>()
                    .unwrap(),
                fn_name: cap.name("fn_name").unwrap().as_str().to_owned(),
                fn_type: cap.name("fn_type").unwrap().as_str().parse::<u8>().unwrap(),
                inc_file_name: cap.name("inc_file_name").unwrap().as_str().to_owned(),
                file_name: cap.name("file_name").unwrap().as_str().to_owned(),
                line_num: cap
                    .name("line_num")
                    .unwrap()
                    .as_str()
                    .parse::<u32>()
                    .unwrap(),
                arg_num: cap
                    .name("arg_num")
                    .unwrap()
                    .as_str()
                    .parse::<u32>()
                    .unwrap(),
                args: cap.name("args").unwrap().as_str().to_owned(),
            };
        }
    }
    #[allow(unused)]
    #[derive(Clone, Debug)]
    struct XtraceEntryRecord {
        rec_type: RecType,
        level: u32,
        fn_num: u32,
        time_idx: f64,
        mem_usage: u32,
        fn_name: String,
        fn_type: u8,
        inc_file_name: String,
        file_name: String,
        line_num: u32,
        arg_num: u32,
        args: String,
    }

    impl XtraceFn for XtraceExitRecord {}
    impl XtraceRecord for XtraceExitRecord {
        fn new(line: &str) -> Self {
            let re = Regex::new(LineRegex::FunctionExit.regex_str()).unwrap();
            let cap = re.captures(line).ok_or("oops").unwrap();
            return XtraceExitRecord {
                rec_type: RecType::Exit,
                level: cap.name("level").unwrap().as_str().parse::<u32>().unwrap(),
                fn_num: cap.name("fn_num").unwrap().as_str().parse::<u32>().unwrap(),
                time_idx: cap
                    .name("time_idx")
                    .unwrap()
                    .as_str()
                    .parse::<f64>()
                    .unwrap(),
                mem_usage: cap
                    .name("mem_usage")
                    .unwrap()
                    .as_str()
                    .parse::<u32>()
                    .unwrap(),
            };
        }
    }
    #[allow(unused)]
    #[derive(Clone, Debug)]
    struct XtraceExitRecord {
        level: u32,
        fn_num: u32,
        rec_type: RecType,
        time_idx: f64,
        mem_usage: u32,
    }

    // Not yet implemented
    /*    struct XtraceReturnRecord {
        level: u32,
        fn_num: u32,
        rec_type: RecType,
        ret_val: u32, // Need to confirm this type. I have yet to see an example to work from and the docs aren't specific.
    }*/

    enum LineRegex {
        Version,
        Format,
        Start,
        FunctionEntry,
        FunctionExit,
        End,
        Penultimate,
    }
    impl LineRegex {
        fn regex_str(&self) -> &str {
            match self {
                LineRegex::Version => r"Version:\s+(?P<version>\d+\.\d+\.\d+).*",
                LineRegex::Format => r"^File format: (?P<format>\d+)",
                LineRegex::Start => {
                    r"^TRACE START \[(?P<start>\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}.\d+)\]"
                }
                LineRegex::FunctionEntry => {
                    r"^(?P<level>\d+)\t(?P<fn_num>\d+)\t(?P<rec_type>0)\t(?P<time_idx>\d+\.\d+)\t(?P<mem_usage>\d+)\t(?P<fn_name>.*)\t(?P<fn_type>[01])\t(?P<inc_file_name>.*)\t(?P<file_name>.*)\t(?P<line_num>\d+)\t(?P<arg_num>\d+)\t?(?P<args>.*)"
                }
                LineRegex::FunctionExit => {
                    r"^(?P<level>\d+)\t(?P<fn_num>\d+)\t(?P<rec_type>1)\t(?P<time_idx>\d+\.\d+)\t(?P<mem_usage>\d+).*"
                }
                LineRegex::Penultimate => r"^\s+(?P<time_idx>\d+\.\d+)\t(?P<mem_usage>\d+)",
                LineRegex::End => {
                    r"^TRACE END\s+\[(?P<end>\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}.\d+)\]"
                }
            }
        }
    }
}
