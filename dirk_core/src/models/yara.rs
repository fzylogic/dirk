use serde::{de, Deserialize, Serialize};
use serde_json::value::Value;
use std::fmt;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[allow(non_camel_case_types)]
pub enum Action {
    clean,
    disable,
    #[default]
    ignore,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[allow(non_camel_case_types)]
pub enum Priority {
    #[default]
    high,
    medium,
}

/// Severity associated with a single rule
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[allow(non_camel_case_types)]
pub enum Severity {
    #[default]
    red,
    yellow,
}

/// Type of file targeted by the rule
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[allow(non_camel_case_types)]
pub enum Target {
    #[default]
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

/// Not-yet-used classification of a scanned script/application
#[derive(Clone, Debug, Deserialize)]
pub enum Type {
    Backdoor,
}

/// Definition of a legacy Hank detection rule
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

pub type ResultStatus = crate::models::dirk::DirkResultClass;

impl fmt::Display for ResultStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResultStatus::OK => write!(f, "OK"),
            ResultStatus::Bad => write!(f, "BAD"),
            ResultStatus::Inconclusive => write!(f, "Inconclusive"),
        }
    }
}

/// The result of a single file scan
#[derive(Debug, Default, Serialize)]
pub struct ScanResult {
    pub filename: PathBuf,
    pub signature: Option<Signature>,
    pub status: ResultStatus,
}

/// Used to deserialize the loosely-defined booleans in our signatures.json (and other) files
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
