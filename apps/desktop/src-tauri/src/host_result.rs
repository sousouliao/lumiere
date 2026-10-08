//! Validate native output before projecting it into product/UI state.
use lumiere_capture_contract::{CaptureMode, Delivery, DeliveryTarget, DynamicRange, Failure};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Capabilities {
    contract_version: u8,
    platform: Windows,
    host_status: HostStatus,
    capture_modes: Vec<CaptureMode>,
    delivery_targets: Vec<DeliveryTarget>,
    hdr_capture: HdrStatus,
    output_profiles: Vec<Profile>,
    unavailable_reason: Option<Failure>,
}
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Windows {
    Windows,
}
#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum HostStatus {
    Available,
    Unavailable,
}
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum HdrStatus {
    Supported,
    Unavailable,
    Unvalidated,
}
#[derive(Deserialize)]
enum Profile {
    #[serde(rename = "srgb-visual-match")]
    SrgbVisualMatch,
}
#[derive(Deserialize)]
#[serde(tag = "status", rename_all = "lowercase", deny_unknown_fields)]
enum Capture {
    #[serde(rename_all = "camelCase")]
    Completed {
        source_dynamic_range: DynamicRange,
        output_profile: Profile,
        deliveries: Vec<Value>,
    },
    Cancelled,
    Failed {
        failure: Failure,
    },
}

pub fn validate(method: &str, version: u8, params: &Value, result: &Value) -> Result<(), String> {
    let valid = match method {
        "getCapabilities" => serde_json::from_value::<Capabilities>(result.clone())
            .map(|caps| {
                let _ = (caps.platform, caps.hdr_capture);
                caps.contract_version == version
                    && caps.output_profiles.len() == 1
                    && unique(&caps.capture_modes)
                    && unique(&caps.delivery_targets)
                    && ((caps.host_status == HostStatus::Available
                        && caps.unavailable_reason.is_none())
                        || (caps.host_status == HostStatus::Unavailable
                            && caps.unavailable_reason.is_some()
                            && caps.capture_modes.is_empty()
                            && caps.delivery_targets.is_empty()))
            })
            .unwrap_or(false),
        "captureDisplay" | "captureRegion" => serde_json::from_value::<Capture>(result.clone())
            .map(|capture| match capture {
                Capture::Completed {
                    source_dynamic_range,
                    output_profile,
                    deliveries,
                } => {
                    let _ = (source_dynamic_range, output_profile);
                    let Ok(delivery) =
                        serde_json::from_value::<Delivery>(params["delivery"].clone())
                    else {
                        return false;
                    };
                    let mut targets = Vec::new();
                    for item in &deliveries {
                        let Ok(target) =
                            serde_json::from_value::<DeliveryTarget>(item["target"].clone())
                        else {
                            return false;
                        };
                        let Some(object) = item.as_object() else {
                            return false;
                        };
                        let valid = match item["status"].as_str() {
                            Some("success") => {
                                if target == DeliveryTarget::Clipboard {
                                    object.len() == 2
                                } else {
                                    object.len() == 3
                                        && item["filePath"].as_str().is_some_and(|path| {
                                            std::path::Path::new(path).is_absolute()
                                        })
                                }
                            }
                            Some("failed") => {
                                object.len() == 3
                                    && serde_json::from_value::<Failure>(item["failure"].clone())
                                        .is_ok()
                            }
                            _ => false,
                        };
                        if !valid {
                            return false;
                        }
                        targets.push(target);
                    }
                    targets.len() == delivery.targets().len()
                        && unique(&targets)
                        && delivery
                            .targets()
                            .iter()
                            .all(|target| targets.contains(target))
                }
                Capture::Cancelled => true,
                Capture::Failed { failure } => {
                    let _ = failure;
                    true
                }
            })
            .unwrap_or(false),
        "cancelRegion" => result
            .as_object()
            .is_some_and(|object| object.len() == 1 && result["status"] == "released"),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err("Native Host result violates the Windows contract".into())
    }
}
fn unique<T: PartialEq>(values: &[T]) -> bool {
    values
        .iter()
        .enumerate()
        .all(|(index, value)| !values[..index].contains(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn rejects_incomplete_delivery_and_false_capabilities() {
        let mut capture = json!({"status":"completed","sourceDynamicRange":"hdr","outputProfile":"srgb-visual-match","deliveries":[{"target":"clipboard","status":"success"}]});
        assert!(
            validate(
                "captureRegion",
                6,
                &json!({"delivery":"clipboard"}),
                &capture
            )
            .is_ok()
        );
        assert!(validate("captureRegion", 6, &json!({"delivery":"both"}), &capture).is_err());
        capture["deliveries"][0]["filePath"] = json!("C:\\invalid.png");
        assert!(
            validate(
                "captureRegion",
                6,
                &json!({"delivery":"clipboard"}),
                &capture
            )
            .is_err()
        );
        let caps = json!({"contractVersion":5,"platform":"windows","hostStatus":"unavailable","captureModes":["region"],"deliveryTargets":[],"hdrCapture":"unavailable","outputProfiles":["srgb-visual-match"]});
        assert!(validate("getCapabilities", 5, &json!({}), &caps).is_err());
    }
}
