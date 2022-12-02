pub mod phpxdebug {
    use std::path::PathBuf;
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
        MemUsage: usize,
        FnName: String,
        fn_type: FnType,
        inc_file_name: String,
        filename: PathBuf,
        line_num: usize,
        arg_num: usize,
        args: Vec<String>,
    }
    struct XtraceExitRecord {
        Level: usize,
        FnNum: usize,
        RecType: RecType,
        TimeIdx: f64,
        MemUsage: usize,
    }
    struct XtraceReturnRecord {
        Level: usize,
        FnNum: usize,
        rec_type: RecType,
        RetVal: usize, // Need to confirm this type. I have yet to see an example to work from and the docs aren't specific.
    }
}