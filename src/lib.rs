pub mod phpxdebug {
    use std::collections::HashMap;
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
        Return,
    }
    enum FnType {
        Internal,
        User,
    }
    struct XtraceEntryRecord {
        level: usize,
        fn_num: usize,
        rec_type: RecType,
        time_idx: f64,
        mem_usage: usize,
        fn_name: String,
        fn_type: FnType,
        inc_file_name: String,
        filename: PathBuf,
        line_num: usize,
        arg_num: usize,
        args: Vec<String>,
    }

    enum XtraceParseError {
        ParseError,
    }

    impl XtraceEntryRecord {
        fn from_string(line: &String) -> Result<XtraceEntryRecord, ParseError> {
            let re = LineRegex::function.regex();
            if let Some(cap) = re.captures(line) {
                return Ok(XtraceEntryRecord {
                    level: cap.name("level").unwrap().as_str().parse::<usize>()?,
                    fn_num: cap.name("fn_num").into()?,
                    rec_type: cap.name("rec_type").into()?,
                    time_idx: cap.name("time_idx").into()?,
                    mem_usage: cap.name("mem_usage").into()?,
                    fn_name: cap.name("fn_name").into()?,
                    fn_type: cap.name("fn_type").into()?,
                    inc_file_name: cap.name("inc_file_name").into()?,
                    filename: cap.name("filename").into()?,
                    line_num: cap.name("line_num").into()?,
                    arg_num: cap.name("arg_num").into()?,
                    args: cap.name("args").into()?,
                })
            } else {
                return Err("oops");
            }
        }
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

/*    fn prefix_to_regex(prefix: &String) -> Result<LineRegex, &'static str> {
        if prefix == "Version:" {
            return Ok(LineRegex::version);
        } else if prefix == "File" {
            return Ok(LineRegex::format);
        } else if prefix == "TRACE" {
            return Ok(LineRegex::start);
        } else {
            let int_check = prefix.sta
            match int_check.is_ok() {
                Some(_result) => return Ok(LineRegex::function),
                None => return Err("Unknown prefix"),
            }
        }
    }*/

    pub fn parse_xtrace_file(file: String) -> Result<Vec<XtraceFnRecord>, std::io::Error> {
        let xtrace_file = File::open(file)?;
        let mut reader = BufReader::new(xtrace_file);
        let mut line = String::new();
        let mut line_num = 0;
        loop {
            line_num += 1;
            if let Ok(len) = reader.read_line(&mut line) {
                if len == 0 {
                    break;
                }
                if line_num == 1 {
                    let re = LineRegex::version.regex();
                    if let Some(cap) = re.captures(&line) {
                        let version = cap.name("version").expect("Didn't find the version line where we expected it");
                        println!("Version: {}", version.as_str());
                    }
                    //Nothing for now
                } else if line_num == 2 {
                    let re = LineRegex::format.regex();
                    if let Some(cap) = re.captures(&line) {
                        let format = cap.name("format").expect("Didn't find the file format line where we expected it").as_str();
                        assert!(SUPPORTED_FILE_FORMATS.contains(&format.parse::<u8>().unwrap()));
                    }
                } else if line_num == 3 {
                    let re = LineRegex::start.regex();
                    if let Some(cap) = re.captures(&line) {
                        let _start = cap.name("format").expect("Didn't find the start time line where we expected it").as_str();
                    }
                } else {
                    let re = LineRegex::function.regex();
                    if let Some(cap) = re.captures(&line) {

                    }
                }
            } else {
                break;
            }
        }
        Err(std::io::Error::new(ErrorKind::Other, "not implemented"))
    }
}
