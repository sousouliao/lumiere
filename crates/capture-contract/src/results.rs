//! Wire result types and semantic validation shared by the native engine and shell.
use crate::{CaptureMode, CaptureParams, DeliveryTarget, Failure, optional_string};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Windows,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HostStatus {
    Available,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HdrCapture {
    Supported,
    Unavailable,
    Unvalidated,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum OutputProfile {
    #[serde(rename = "srgb-visual-match")]
    SrgbVisualMatch,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Capabilities {
    pub contract_version: u8,
    pub platform: Platform,
    pub host_status: HostStatus,
    pub capture_modes: Vec<CaptureMode>,
    pub delivery_targets: Vec<DeliveryTarget>,
    pub hdr_capture: HdrCapture,
    pub output_profiles: Vec<OutputProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<Failure>,
}

impl Capabilities {
    pub fn unavailable(version: u8, reason: Failure) -> Self {
        Self {
            contract_version: version,
            platform: Platform::Windows,
            host_status: HostStatus::Unavailable,
            capture_modes: vec![],
            delivery_targets: vec![],
            hdr_capture: HdrCapture::Unavailable,
            output_profiles: vec![OutputProfile::SrgbVisualMatch],
            unavailable_reason: Some(reason),
        }
    }

    fn valid(&self, version: u8) -> bool {
        self.contract_version == version
            && self.output_profiles.len() == 1
            && unique(&self.capture_modes)
            && unique(&self.delivery_targets)
            && match self.host_status {
                HostStatus::Available => self.unavailable_reason.is_none(),
                HostStatus::Unavailable => {
                    self.unavailable_reason.is_some()
                        && self.capture_modes.is_empty()
                        && self.delivery_targets.is_empty()
                }
            }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DynamicRange {
    Sdr,
    Hdr,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "status", rename_all = "lowercase", deny_unknown_fields)]
pub enum CaptureOutcome {
    #[serde(rename_all = "camelCase")]
    Completed {
        source_dynamic_range: DynamicRange,
        output_profile: OutputProfile,
        deliveries: Vec<DeliveryResult>,
    },
    Cancelled,
    Failed {
        failure: Failure,
    },
}

#[derive(Debug, Serialize)]
pub struct DeliveryResult {
    pub target: DeliveryTarget,
    #[serde(flatten)]
    pub outcome: DeliveryOutcome,
}

// Deserialize target separately so the tagged outcome can reject unknown fields.
// Serde's flatten + deny_unknown_fields combination cannot enforce that contract.
impl<'de> Deserialize<'de> for DeliveryResult {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut object = serde_json::Map::<String, Value>::deserialize(deserializer)?;
        let target = object
            .remove("target")
            .ok_or_else(|| serde::de::Error::missing_field("target"))?;
        Ok(Self {
            target: serde_json::from_value(target).map_err(serde::de::Error::custom)?,
            outcome: serde_json::from_value(Value::Object(object))
                .map_err(serde::de::Error::custom)?,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "status", rename_all = "lowercase", deny_unknown_fields)]
pub enum DeliveryOutcome {
    #[serde(rename_all = "camelCase")]
    Success {
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "optional_string"
        )]
        file_path: Option<String>,
    },
    Failed {
        failure: Failure,
    },
}

impl DeliveryResult {
    fn valid(&self) -> bool {
        match &self.outcome {
            DeliveryOutcome::Success { file_path } => match self.target {
                DeliveryTarget::Clipboard => file_path.is_none(),
                DeliveryTarget::Folder => file_path
                    .as_deref()
                    .is_some_and(|path| std::path::Path::new(path).is_absolute()),
            },
            DeliveryOutcome::Failed { .. } => true,
        }
    }
}

/// Only this decoded, validated result crosses from the JSONL adapter into the shell.
#[derive(Debug)]
pub enum HostResult {
    Capabilities(Capabilities),
    Capture(CaptureOutcome),
    Released,
}

#[derive(Deserialize)]
#[serde(tag = "status", rename_all = "lowercase", deny_unknown_fields)]
enum Released {
    Released {},
}

impl HostResult {
    pub fn decode(
        method: &str,
        version: u8,
        params: &Value,
        result: Value,
    ) -> Result<Self, &'static str> {
        let invalid = "Native Host result violates the Windows contract";
        if ![5, 6].contains(&version) {
            return Err(invalid);
        }
        match method {
            "getCapabilities" => {
                let caps: Capabilities = serde_json::from_value(result).map_err(|_| invalid)?;
                if !caps.valid(version) {
                    return Err(invalid);
                }
                Ok(Self::Capabilities(caps))
            }
            "captureDisplay" | "captureRegion" if method != "captureRegion" || version == 6 => {
                if result["status"] == "cancelled"
                    && result.as_object().is_none_or(|object| object.len() != 1)
                {
                    return Err(invalid);
                }
                let capture: CaptureOutcome =
                    serde_json::from_value(result).map_err(|_| invalid)?;
                if let CaptureOutcome::Completed { deliveries, .. } = &capture {
                    let params: CaptureParams =
                        serde_json::from_value(params.clone()).map_err(|_| invalid)?;
                    let targets: Vec<_> = deliveries.iter().map(|item| item.target).collect();
                    if targets.len() != params.delivery.targets().len()
                        || !unique(&targets)
                        || !params
                            .delivery
                            .targets()
                            .iter()
                            .all(|target| targets.contains(target))
                        || !deliveries.iter().all(DeliveryResult::valid)
                    {
                        return Err(invalid);
                    }
                }
                Ok(Self::Capture(capture))
            }
            "cancelRegion" if version == 6 => {
                serde_json::from_value::<Released>(result).map_err(|_| invalid)?;
                Ok(Self::Released)
            }
            _ => Err(invalid),
        }
    }
}

fn unique<T: PartialEq>(values: &[T]) -> bool {
    values
        .iter()
        .enumerate()
        .all(|(index, value)| !values[..index].contains(value))
}
