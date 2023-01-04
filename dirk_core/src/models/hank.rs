use crate::dirk_api;
use serde::{de, Deserialize, Serialize};
use serde_json::value::Value;
use std::fmt;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[allow(non_camel_case_types)]
pub enum Action {
    clean,
    disable,
    ignore,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[allow(non_camel_case_types)]
pub enum Priority {
    high,
    medium,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[allow(non_camel_case_types)]
pub enum Severity {
    red,
    yellow,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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

#[derive(Clone, Debug, Deserialize)]
pub enum Type {
    Backdoor,
}

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

pub type ResultStatus = crate::dirk_api::DirkResultClass;

impl fmt::Display for ResultStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResultStatus::OK => write!(f, "OK"),
            ResultStatus::Bad => write!(f, "BAD"),
            ResultStatus::Inconclusive => write!(f, "Inconclusive"),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ScanResult {
    pub filename: PathBuf,
    pub signature: Option<Signature>,
    pub status: ResultStatus,
}

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
