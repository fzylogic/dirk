pub mod phpxdebug {
    use std::collections::HashMap;
    use std::error::Error;
    use regex;
    use std::fs::File;
    use std::io::{BufReader, ErrorKind};
    use std::io::prelude::*;
    use std::path::PathBuf;
    use std::str::FromStr;
    use std::string::ParseError;
    use regex::Regex;

    enum RecType {
        Entry,
        Exit,
        Format,
        Return,
        StartTime,
        Version,
    }
    trait XtraceRecord {
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
    pub struct XtraceFnRecord {
        fn_num: usize,
        entry_record: XtraceEntryRecord,
        exit_record: Option<XtraceExitRecord>,
        return_record: Option<XtraceReturnRecord>
    }
    impl XtraceEntryRecord {
        fn from_string(line: &String) -> Result<XtraceEntryRecord, Box<dyn Error>> {
            let re = LineRegex::function.regex();
            let cap = re.captures(line).ok_or("oops")?;
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
    }

    static SUPPORTED_FILE_FORMATS: &[u8] = &[4];

    enum LineRegex {
        version,
        format,
        start,
        function,
    }

    impl LineRegex {
        fn regex(&self) -> Regex {
            match self {
                LineRegex::version => Regex::new(r"^Version: ?P<version>(\d+.\d+.\d+)").unwrap(),
                LineRegex::format => Regex::new(r"^File format: ?P<format>(\d+)").unwrap(),
                LineRegex::start => Regex::new(r"^TRACE START \[?P<start>\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}.\d+)\]").unwrap(),
                LineRegex::function => Regex::new(r"^?P<level>(\d+)\t?P<fn_num>(\d+)\t?P<rec_type>([AR01])\t?P<time_idx>(\d+\.\d+)\t?P<mem_usage>(\d+)\t?P<fn_name>(.*)\t?P<fn_type>([01])\t?P<inc_file_name>(.*)\t?P<filename>(.*)\t?P<line_num>(\d+)\t?P<arg_num>(\d+)\t?P<args>(.*)$").unwrap(),
            }
        }
    }

    fn line_to_record(line: &String) -> impl XtraceRecord {

    }

    pub fn parse_xtrace_file(file: String) -> Result<Vec<XtraceFnRecord>, std::io::Error> {
        let xtrace_file = File::open(file)?;
        let mut reader = BufReader::new(xtrace_file);
        let mut line = String::new();
        loop {
            break;
        }
        Err(std::io::Error::new(ErrorKind::Other, "not implemented"))
    }
}
