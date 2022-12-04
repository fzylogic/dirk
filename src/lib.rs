pub mod phpxdebug {

    use regex;
    use std::fs::File;
    use std::io::prelude::*;
    use std::io::{BufReader, ErrorKind};
    use std::path::PathBuf;

    use regex::RegexSet;

    enum RecType {
        Entry,
        Exit,
        Format,
        Return,
        StartTime,
        Version,
    }
    trait XtraceRecord {
        fn new(pattern: LineRegex) -> Self;
    }
    pub struct XtraceRun {
        id: uuid::Uuid,
        fn_records: Vec<XtraceFnRecord>,
    }
    impl XtraceRecord for XtraceFnRecord {
        fn new(_pattern: LineRegex) -> XtraceFnRecord {
            XtraceFnRecord {
                fn_num: 1,
                entry_record: None,
                exit_record: None,
                return_record: None,
            }
        }
    }
    pub struct XtraceFnRecord {
        fn_num: usize,
        entry_record: Option<XtraceEntryRecord>,
        exit_record: Option<XtraceExitRecord>,
        return_record: Option<XtraceReturnRecord>,
    }
    impl XtraceRecord for XtraceVersionRecord {
        fn new(_pattern: LineRegex) -> XtraceVersionRecord {
            XtraceVersionRecord { version: "3.1.6" }
        }
    }
    pub struct XtraceVersionRecord {
        version: &'static str,
    }

    impl XtraceRecord for XtraceFmtRecord {
        fn new(_pattern: LineRegex) -> XtraceFmtRecord {
            XtraceFmtRecord { format: 4 }
        }
    }
    pub struct XtraceFmtRecord {
        format: usize,
    }
    enum FnType {
        Internal,
        User,
    }

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

    struct XtraceExitRecord {
        level: usize,
        fn_num: usize,
        rec_type: RecType,
        time_idx: f64,
        mem_usage: usize,
    }
    struct XtraceReturnRecord {
        level: usize,
        fn_num: usize,
        rec_type: RecType,
        ret_val: usize, // Need to confirm this type. I have yet to see an example to work from and the docs aren't specific.
    }

    /*   impl XtraceEntryRecord {
        fn from_string(line: &String) -> Result<XtraceEntryRecord, Box<dyn Error>> {
            //let re = LineRegex::function.regex();
            //let cap = re.captures(line).ok_or("oops")?;
            return Ok(XtraceEntryRecord {
                rec_type: RecType::Entry,
                level: cap.name("level").ok_or("oops")?.as_str().parse::<usize>()?,
                fn_num: cap.name("fn_num").ok_or("oops")?.as_str().parse::<usize>()?,
                time_idx: cap.name("time_idx").ok_or("oops")?.as_str().parse::<f64>()?,
                mem_usage: cap.name("mem_usage").ok_or("oops")?.as_str().parse::<usize>()?,
                fn_name: cap.name("fn_name").ok_or("oops")?.as_str().to_owned(),
                fn_type: cap.name("fn_type").ok_or("oops")?.as_str().parse::<u8>()?,
                inc_file_name: cap.name("inc_file_name").ok_or("oops")?.as_str().to_owned(),
                filename: PathBuf::from(cap.name("filename").ok_or("oops")?.as_str().to_owned()),
                line_num: cap.name("line_num").ok_or("oops")?.as_str().parse::<usize>()?,
                arg_num: cap.name("arg_num").ok_or("oops")?.as_str().parse::<usize>()?,
                args: cap.name("args").ok_or("oops")?.as_str().to_owned(),
            });
        }
    }*/

    static SUPPORTED_FILE_FORMATS: &[u8] = &[4];

    enum LineRegex {
        version,
        format,
        start,
        function,
    }

    impl LineRegex {
        fn regex_str(&self) -> &str {
            match self {
                LineRegex::version => r"^Version: ?P<version>(\d+.\d+.\d+)",
                LineRegex::format => r"^File format: ?P<format>(\d+)",
                LineRegex::start => {
                    r"^TRACE START \[?P<start>\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}.\d+)\]"
                }
                LineRegex::function => {
                    r"^?P<level>(\d+)\t?P<fn_num>(\d+)\t?P<rec_type>([AR01])\t?P<time_idx>(\d+\.\d+)\t?P<mem_usage>(\d+)\t?P<fn_name>(.*)\t?P<fn_type>([01])\t?P<inc_file_name>(.*)\t?P<filename>(.*)\t?P<line_num>(\d+)\t?P<arg_num>(\d+)\t?P<args>(.*)$"
                }
            }
        }
    }

    fn process_line(_run: XtraceRun, line: &String) -> impl XtraceRecord {
        let set = RegexSet::new([
            LineRegex::version.regex_str(),
            LineRegex::format.regex_str(),
            LineRegex::start.regex_str(),
            LineRegex::function.regex_str(),
        ])
        .unwrap_or_else(|_| panic!("Failed to parse line '{}'", line));
        let matches: Vec<_> = set.matches(line.as_str()).into_iter().collect();
        assert_eq!(matches.len(), 1);
        let _idx = matches.first().unwrap();
        XtraceFmtRecord { format: 4 }
    }

    pub fn parse_xtrace_file(
        id: uuid::Uuid,
        file: String,
    ) -> Result<Vec<XtraceFnRecord>, std::io::Error> {
        let xtrace_file = File::open(file)?;
        let mut reader = BufReader::new(xtrace_file);
        let mut line = String::new();
        let _run = XtraceRun {
            id,
            fn_records: Vec::new(),
        };
        loop {
            reader.read_line(&mut line).unwrap();
            break;
        }
        Err(std::io::Error::new(ErrorKind::Other, "not implemented"))
    }
}
