use std::{collections::BTreeMap, error::Error, fmt};

use serde::Serialize;
use serde_json::Value;

pub(crate) type Result<T> = std::result::Result<T, TimeKeepError>;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
pub(crate) enum ErrorCode {
    InvalidParams,
    Io,
    Internal,
}

impl ErrorCode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ErrorCode::InvalidParams => "INVALID_PARAMS",
            ErrorCode::Io => "IO_ERROR",
            ErrorCode::Internal => "INTERNAL_ERROR",
        }
    }
}

#[derive(Debug)]
pub(crate) struct TimeKeepError {
    code: ErrorCode,
    message: String,
    details: BTreeMap<String, Value>,
}

impl TimeKeepError {
    pub(crate) fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: BTreeMap::new(),
        }
    }

    pub(crate) fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidParams, message)
    }

    pub(crate) fn code(&self) -> ErrorCode {
        self.code
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }

    pub(crate) fn details(&self) -> &BTreeMap<String, Value> {
        &self.details
    }

    pub(crate) fn with_detail(mut self, key: impl Into<String>, value: Value) -> Self {
        self.details.insert(key.into(), value);
        self
    }
}

impl fmt::Display for TimeKeepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}

impl Error for TimeKeepError {}

impl From<std::io::Error> for TimeKeepError {
    fn from(err: std::io::Error) -> Self {
        Self::new(ErrorCode::Io, err.to_string())
    }
}

impl From<csv::Error> for TimeKeepError {
    fn from(err: csv::Error) -> Self {
        Self::new(ErrorCode::Internal, err.to_string())
    }
}

impl From<serde_json::Error> for TimeKeepError {
    fn from(err: serde_json::Error) -> Self {
        Self::new(ErrorCode::Internal, err.to_string())
    }
}

impl From<rusqlite::Error> for TimeKeepError {
    fn from(err: rusqlite::Error) -> Self {
        Self::new(ErrorCode::Internal, err.to_string())
    }
}
