use lumiere_capture_contract::*;
use serde_json::{Value, json};

#[test]
fn current_windows_requests_conform() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../protocol/platform-host/fixtures");
    for (version, files) in [
        (
            5,
            vec![
                "get-capabilities.request.json",
                "capture-display.request.json",
            ],
        ),
        (
            6,
            vec![
                "get-capabilities.request.json",
                "capture-display.request.json",
                "capture-region.request.json",
                "cancel-region.request.json",
            ],
        ),
    ] {
        for file in files {
            let mut value: Value = serde_json::from_str(
                &std::fs::read_to_string(root.join(format!("v{version}")).join(file)).unwrap(),
            )
            .unwrap();
            // Fixtures may carry Mac-native paths; requests exercised on Windows use Windows paths.
            if value["params"].get("saveDirectory").is_some() {
                value["params"]["saveDirectory"] = json!("C:\\Pictures\\Lumiere");
            }
            let request = Request::parse(&value.to_string()).unwrap();
            assert_eq!(request.version, version);
        }
    }
}

#[test]
fn malformed_inputs_keep_correlation_and_reject_unknown_properties() {
    for value in [
        json!({"version":6,"id":"bad","method":"getCapabilities","params":{"extra":true}}),
        json!({"version":6,"id":"bad","method":"captureRegion","params":{"delivery":"clipboard","saveDirectory":"C:\\Pictures"}}),
        json!({"version":6,"id":"bad","method":"captureRegion","params":{"delivery":"folder","saveDirectory":null}}),
        json!({"version":6,"id":"bad","method":"captureRegion","params":{"delivery":"folder","saveDirectory":"relative"}}),
        json!({"version":6,"id":"bad","method":"cancelRegion","params":{"requestId":""}}),
        json!({"version":6,"id":"bad","method":"captureRegion","params":{"delivery":"unknown"}}),
        json!({"version":6,"id":"bad","method":"getCapabilities","params":{},"extra":true}),
    ] {
        let response = Request::parse(&value.to_string()).unwrap_err();
        assert_eq!(response.version, 6);
        assert_eq!(response.id, "bad");
        assert!(matches!(
            response.payload,
            ResponsePayload::Error(Failure {
                code: FailureCode::InvalidRequest,
                ..
            })
        ));
    }
    assert!(
        Request::parse(
            r#"{"version":6,"version":5,"id":"x","method":"getCapabilities","params":{}}"#
        )
        .is_err()
    );
    assert!(Request::parse("{broken").is_err());
}

#[test]
fn unsupported_platform_and_legacy_methods_are_rejected() {
    for (version, method) in [
        (7, "getCapabilities"),
        (5, "captureRegion"),
        (5, "prepareRegion"),
        (6, "requestScreenCapturePermission"),
    ] {
        assert!(
            Request::parse(
                &json!({"version":version,"id":"x","method":method,"params":{}}).to_string()
            )
            .is_err()
        );
    }
}

#[test]
fn partial_delivery_preserves_the_successful_target() {
    let result = CaptureOutcome::Completed {
        source_dynamic_range: DynamicRange::Hdr,
        output_profile: "srgb-visual-match",
        deliveries: vec![
            DeliveryResult {
                target: DeliveryTarget::Clipboard,
                outcome: DeliveryOutcome::Success { file_path: None },
            },
            DeliveryResult {
                target: DeliveryTarget::Folder,
                outcome: DeliveryOutcome::Failed {
                    failure: Failure {
                        code: FailureCode::DeliveryFailed,
                        message: "Folder unavailable".into(),
                        retryable: true,
                    },
                },
            },
        ],
    };
    let response = serde_json::to_value(Response::result(5, "capture".into(), result)).unwrap();
    assert_eq!(response["result"]["status"], "completed");
    assert_eq!(
        response["result"]["deliveries"][0],
        json!({"target":"clipboard","status":"success"})
    );
    assert_eq!(response["result"]["deliveries"][1]["status"], "failed");
    assert!(response.get("error").is_none());
}
