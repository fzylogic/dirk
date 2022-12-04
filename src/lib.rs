pub mod phpxdebug {
    use lazy_static::lazy_static;
    use regex;
    use std::fs::File;
    use std::io::prelude::*;
    use std::io::{BufReader, ErrorKind};
    use std::path::PathBuf;

    use regex::{Regex, RegexSet};

    #[derive(Clone, Debug)]
    enum RecType {
        Entry,
        Exit,
        Format,
        Return,
        StartTime,
        Version,
    }
    trait XtraceRecord {
        fn new(line: &str) -> Self;
    }
    trait XtraceFn {}
    #[allow(unused)]
    #[derive(Clone, Debug)]
    pub struct XtraceRun {
        id: uuid::Uuid,
        start: Option<XtraceStartTimeRecord>,
        format: Option<XtraceFmtRecord>,
        version: Option<XtraceVersionRecord>,
        fn_records: Vec<XtraceFnRecord>,
    }
    impl XtraceRun {
        fn add_fn_record(&mut self, _func: impl XtraceFn) {}
    }
    #[allow(unused)]
    #[derive(Clone, Debug)]
    pub struct XtraceFnRecord {
        fn_num: usize,
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
                        .parse::<usize>()
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
        format: usize,
        rec_type: RecType,
    }
    enum FnType {
        Internal,
        User,
    }

    impl XtraceFn for XtraceEntryRecord {}
    impl XtraceRecord for XtraceEntryRecord {
        fn new(line: &str) -> Self {
            let re = Regex::new(LineRegex::Function_Entry.regex_str()).unwrap();
            let cap = re.captures(line).ok_or("oops").unwrap();
            return XtraceEntryRecord {
                rec_type: RecType::Entry,
                level: cap
                    .name("level")
                    .unwrap()
                    .as_str()
                    .parse::<usize>()
                    .unwrap(),
                fn_num: cap
                    .name("fn_num")
                    .unwrap()
                    .as_str()
                    .parse::<usize>()
                    .unwrap(),
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
                    .parse::<usize>()
                    .unwrap(),
                fn_name: cap.name("fn_name").unwrap().as_str().to_owned(),
                fn_type: cap.name("fn_type").unwrap().as_str().parse::<u8>().unwrap(),
                inc_file_name: cap.name("inc_file_name").unwrap().as_str().to_owned(),
                filename: PathBuf::from(cap.name("filename").unwrap().as_str().to_owned()),
                line_num: cap
                    .name("line_num")
                    .unwrap()
                    .as_str()
                    .parse::<usize>()
                    .unwrap(),
                arg_num: cap
                    .name("arg_num")
                    .unwrap()
                    .as_str()
                    .parse::<usize>()
                    .unwrap(),
                args: cap.name("args").unwrap().as_str().to_owned(),
            };
        }
    }
    #[allow(unused)]
    #[derive(Clone, Debug)]
    struct XtraceEntryRecord {
        rec_type: RecType,
        level: usize,
        fn_num: usize,
        time_idx: f64,
        mem_usage: usize,
        fn_name: String,
        fn_type: u8,
        inc_file_name: String,
        filename: PathBuf,
        line_num: usize,
        arg_num: usize,
        args: String,
    }

    impl XtraceFn for XtraceExitRecord {}
    impl XtraceRecord for XtraceExitRecord {
        fn new(line: &str) -> Self {
            let re = Regex::new(LineRegex::Function_Exit.regex_str()).unwrap();
            let cap = re.captures(line).ok_or("oops").unwrap();
            return XtraceExitRecord {
                rec_type: RecType::Exit,
                level: cap
                    .name("level")
                    .unwrap()
                    .as_str()
                    .parse::<usize>()
                    .unwrap(),
                fn_num: cap
                    .name("fn_num")
                    .unwrap()
                    .as_str()
                    .parse::<usize>()
                    .unwrap(),
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
                    .parse::<usize>()
                    .unwrap(),
            };
        }
    }
    #[allow(unused)]
    #[derive(Clone, Debug)]
    struct XtraceExitRecord {
        level: usize,
        fn_num: usize,
        rec_type: RecType,
        time_idx: f64,
        mem_usage: usize,
    }
    /*    struct XtraceReturnRecord {
        level: usize,
        fn_num: usize,
        rec_type: RecType,
        ret_val: usize, // Need to confirm this type. I have yet to see an example to work from and the docs aren't specific.
    }*/

    static SUPPORTED_FILE_FORMATS: &[&str] = &["4"];

    enum LineRegex {
        Version,
        Format,
        Start,
        Function_Entry,
        Function_Exit,
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
                LineRegex::Function_Entry => {
                    r"^(?P<level>\d+)\t(?P<fn_num>\d+)\t(?P<rec_type>0)\t(?P<time_idx>\d+\.\d+)\t(?P<mem_usage>\d+)\t(?P<fn_name>.*)\t(?P<fn_type>[01])\t(?P<inc_file_name>.*)\t(?P<filename>.*)\t(?P<line_num>\d+)\t(?P<arg_num>\d+)\t?(?P<args>.*)"
                }
                LineRegex::Function_Exit => {
                    r"^(?P<level>\d+)\t(?P<fn_num>\d+)\t(?P<rec_type>1)\t(?P<time_idx>\d+\.\d+)\t(?P<mem_usage>\d+).*"
                }
                LineRegex::Penultimate => r"^\s+(?P<time_idx>\d+\.\d+)\t(?P<mem_usage>\d+)",
                LineRegex::End => {
                    r"^TRACE END \[(?P<end>\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}.\d+)\]"
                }
            }
        }
    }
    lazy_static! {
        static ref RE_SET: regex::RegexSet = RegexSet::new([
            LineRegex::Version.regex_str(),
            LineRegex::Format.regex_str(),
            LineRegex::Start.regex_str(),
            LineRegex::Function_Entry.regex_str(),
            LineRegex::Function_Exit.regex_str(),
            LineRegex::Penultimate.regex_str(),
            LineRegex::End.regex_str(),
        ])
        .unwrap();
    }

    fn process_line(run: &mut XtraceRun, line: &String) {
        let matches: Vec<_> = RE_SET.matches(line.as_str()).into_iter().collect();
        assert_eq!(matches.len(), 1);
        let idx = matches.first().unwrap();
        match idx {
            0 => run.version = Some(XtraceVersionRecord::new(line)),
            1 => run.format = Some(XtraceFmtRecord::new(line)),
            2 => run.start = Some(XtraceStartTimeRecord::new(line)),
            3 => run.add_fn_record(XtraceEntryRecord::new(line)),
            4 => run.add_fn_record(XtraceExitRecord::new(line)),
            _ => todo!(),
        };
    }

    pub fn parse_xtrace_file(id: uuid::Uuid, file: String) -> Result<XtraceRun, std::io::Error> {
        let xtrace_file = File::open(file)?;
        let mut reader = BufReader::new(xtrace_file);
        let mut line = String::new();
        let mut run = XtraceRun {
            id,
            format: None,
            start: None,
            version: None,
            fn_records: Vec::new(),
        };
        let mut line_number: u128 = 1;
        loop {
            let result = reader.read_line(&mut line);
            match result {
                Ok(_size) => {
                    println!("Processing line {line_number}: {line}");
                    process_line(&mut run, &line)
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    continue;
                }
            }
            line_number += 1;
            line.clear();
        }
        Err(std::io::Error::new(ErrorKind::Other, "not implemented"))
    }
}
