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
        Level: usize,
        FnNum: usize,
        RecType: RecType::Entry,
        TimeIdx: f64,
        MemUsage: usize,
        FnName: String,
        FnType: FnType,
        IncFileName: String,
        Filename: PathBuf,
        LineNum: usize,
        ArgNum: usize,
        Args: Vec<String>,
    }
    struct XtraceExitRecord {
        Level: usize,
        FnNum: usize,
        RecType: RecType::Exit,
        TimeIdx: f64,
        MemUsage: usize,
    }
    struct XtraceReturnRecord {
        Level: usize,
        FnNum: usize,
        RetVal: 
    }
}