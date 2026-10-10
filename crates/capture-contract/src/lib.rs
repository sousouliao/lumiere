//! Windows projection of the existing language-neutral v5/v6 Host contract.
//! No platform handles or frame buffers belong on this boundary.

use serde::{Deserialize, Serialize};
use serde_json::Value;

mod results;
pub use results::{
    Capabilities, CaptureOutcome, DeliveryOutcome, DeliveryResult, DynamicRange, HdrCapture,
    HostResult, HostStatus, OutputProfile, Platform,
};

pub const DISPLAY_VERSION: u8 = 5;
pub const REGION_VERSION: u8 = 6;
pub const REGION_LEASE_SECONDS: u64 = 60;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CaptureMode {
    Display,
    Region,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Delivery {
    Clipboard,
    Folder,
    Both,
}

impl Delivery {
    pub fn targets(self) -> &'static [DeliveryTarget] {
        match self {
            Self::Clipboard => &[DeliveryTarget::Clipboard],
            Self::Folder => &[DeliveryTarget::Folder],
            Self::Both => &[DeliveryTarget::Clipboard, DeliveryTarget::Folder],
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DeliveryTarget {
    Clipboard,
    Folder,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureParams {
    pub delivery: Delivery,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_string"
    )]
    pub save_directory: Option<String>,
}

fn optional_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    String::deserialize(d).map(Some)
}

impl CaptureParams {
    pub fn validate(&self) -> Result<(), Failure> {
        if let Some(path) = &self.save_directory
            && (self.delivery == Delivery::Clipboard
                || path.is_empty()
                || !std::path::Path::new(path).is_absolute())
        {
            return Err(Failure::invalid(
                "saveDirectory must be an absolute Windows path and requires folder delivery.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum Operation {
    Capabilities,
    Capture {
        mode: CaptureMode,
        params: CaptureParams,
    },
    CancelRegion {
        request_id: String,
    },
}

#[derive(Debug)]
pub struct Request {
    pub version: u8,
    pub id: String,
    pub operation: Operation,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u8,
    id: String,
    method: String,
    params: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyParams {}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CancelParams {
    request_id: String,
}

impl Request {
    pub fn parse(line: &str) -> Result<Self, Response> {
        let context = serde_json::from_str::<Value>(line).ok();
        let id = context
            .as_ref()
            .and_then(|v| v.get("id"))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .unwrap_or("invalid-request")
            .to_owned();
        let version = if context
            .as_ref()
            .and_then(|v| v.get("version"))
            .and_then(Value::as_u64)
            == Some(6)
        {
            6
        } else {
            5
        };
        let parse = || -> Result<Self, Failure> {
            let envelope: Envelope = serde_json::from_str(line).map_err(|_| {
                Failure::invalid("Request must be one complete, exact JSON object.")
            })?;
            if ![5, 6].contains(&envelope.version) || envelope.id.is_empty() {
                return Err(Failure::invalid("Unsupported version or empty request id."));
            }
            let invalid = |_| Failure::invalid("Invalid request parameters.");
            let operation = match (envelope.version, envelope.method.as_str()) {
                (_, "getCapabilities") => {
                    serde_json::from_value::<EmptyParams>(envelope.params).map_err(invalid)?;
                    Operation::Capabilities
                }
                (_, "captureDisplay") | (6, "captureRegion") => {
                    let params: CaptureParams =
                        serde_json::from_value(envelope.params).map_err(invalid)?;
                    params.validate()?;
                    Operation::Capture {
                        mode: if envelope.method == "captureDisplay" {
                            CaptureMode::Display
                        } else {
                            CaptureMode::Region
                        },
                        params,
                    }
                }
                (6, "cancelRegion") => {
                    let params: CancelParams =
                        serde_json::from_value(envelope.params).map_err(invalid)?;
                    if params.request_id.is_empty() {
                        return Err(Failure::invalid("requestId must not be empty."));
                    }
                    Operation::CancelRegion {
                        request_id: params.request_id,
                    }
                }
                _ => return Err(Failure::invalid("Unknown Windows platform-host method.")),
            };
            Ok(Self {
                version: envelope.version,
                id: envelope.id,
                operation,
            })
        };
        parse().map_err(|failure| Response::error(version, id, failure))
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FailureCode {
    HostUnavailable,
    PermissionDenied,
    CaptureUnavailable,
    DeliveryUnavailable,
    DeliveryFailed,
    InvalidRequest,
    UnexpectedFailure,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Failure {
    pub code: FailureCode,
    pub message: String,
    pub retryable: bool,
}

impl Failure {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: FailureCode::InvalidRequest,
            message: message.into(),
            retryable: false,
        }
    }
    pub fn capture(message: impl Into<String>) -> Self {
        Self {
            code: FailureCode::CaptureUnavailable,
            message: message.into(),
            retryable: true,
        }
    }
    pub fn unexpected(message: impl Into<String>) -> Self {
        Self {
            code: FailureCode::UnexpectedFailure,
            message: message.into(),
            retryable: true,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Response {
    pub version: u8,
    pub id: String,
    #[serde(flatten)]
    pub payload: ResponsePayload,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ResponsePayload {
    Result(Value),
    Error(Failure),
}

impl Response {
    pub fn result(version: u8, id: String, result: impl Serialize) -> Self {
        Self {
            version,
            id,
            payload: ResponsePayload::Result(
                serde_json::to_value(result).expect("typed protocol result serializes"),
            ),
        }
    }
    pub fn error(version: u8, id: String, error: Failure) -> Self {
        Self {
            version,
            id,
            payload: ResponsePayload::Error(error),
        }
    }
    pub fn released(version: u8, id: String) -> Self {
        Self::result(version, id, serde_json::json!({"status":"released"}))
    }
}
