use lumiere_capture_contract::{CaptureOutcome, DeliveryOutcome, HostResult};
use serde_json::{Value, json};

fn completed() -> Value {
    json!({"status":"completed", "sourceDynamicRange":"hdr", "outputProfile":"srgb-visual-match",
        "deliveries":[{"target":"clipboard", "status":"success"}]})
}

#[test]
fn response_fixtures_decode_and_keep_their_wire_shape() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../protocol/platform-host/fixtures");
    for version in [5, 6] {
        let mut fixture: Value = serde_json::from_str(
            &std::fs::read_to_string(
                root.join(format!("v{version}/capture-completed.response.json")),
            )
            .unwrap(),
        )
        .unwrap();
        // File paths are native to the runtime; no artifact is written by this test.
        fixture["result"]["deliveries"][1]["filePath"] =
            json!(std::env::temp_dir().join("lumiere.png"));
        let result = fixture["result"].clone();
        let HostResult::Capture(capture) = HostResult::decode(
            "captureDisplay",
            version,
            &json!({"delivery":"both"}),
            result.clone(),
        )
        .unwrap() else {
            panic!("expected capture result");
        };
        assert_eq!(serde_json::to_value(capture).unwrap(), result);
    }
    for (file, method) in [
        ("capture-region-cancelled.response.json", "captureRegion"),
        ("cancel-region.response.json", "cancelRegion"),
    ] {
        let fixture: Value =
            serde_json::from_str(&std::fs::read_to_string(root.join("v6").join(file)).unwrap())
                .unwrap();
        let decoded = HostResult::decode(
            method,
            6,
            &json!({"delivery":"clipboard"}),
            fixture["result"].clone(),
        )
        .unwrap();
        assert!(matches!(
            decoded,
            HostResult::Capture(CaptureOutcome::Cancelled) | HostResult::Released
        ));
    }
}

#[test]
fn rejects_incomplete_duplicate_or_unrequested_deliveries() {
    let params = json!({"delivery":"both"});
    assert!(HostResult::decode("captureRegion", 6, &params, completed()).is_err());
    let mut duplicate = completed();
    duplicate["deliveries"]
        .as_array_mut()
        .unwrap()
        .push(json!({"target":"clipboard", "status":"success"}));
    assert!(HostResult::decode("captureRegion", 6, &params, duplicate).is_err());
    let mut unrequested = completed();
    unrequested["deliveries"][0] = json!({"target":"folder", "status":"success", "filePath":std::env::temp_dir().join("capture.png")});
    assert!(
        HostResult::decode(
            "captureDisplay",
            5,
            &json!({"delivery":"clipboard"}),
            unrequested
        )
        .is_err()
    );
}

#[test]
fn rejects_unknown_fields_and_invalid_success_fields() {
    for delivery in [
        json!({"target":"clipboard", "status":"success", "filePath":"unexpected"}),
        json!({"target":"clipboard", "status":"success", "filePath":null}),
        json!({"target":"clipboard", "status":"success", "extra":true}),
        json!({"target":"folder", "status":"success"}),
        json!({"target":"folder", "status":"success", "filePath":"relative.png"}),
        json!({"target":"clipboard", "status":"failed", "failure":{"code":"delivery-failed","message":"failed","retryable":true},"filePath":"unexpected"}),
    ] {
        let mut result = completed();
        result["deliveries"][0] = delivery;
        assert!(
            HostResult::decode(
                "captureDisplay",
                5,
                &json!({"delivery":"clipboard"}),
                result
            )
            .is_err()
        );
    }
    for result in [
        json!({"status":"cancelled", "extra":true}),
        json!({"status":"released", "extra":true}),
        json!({"status":"failed", "failure":{"code":"capture-unavailable","message":"failed","retryable":true}, "extra":true}),
    ] {
        let method = if result["status"] == "released" {
            "cancelRegion"
        } else {
            "captureRegion"
        };
        assert!(HostResult::decode(method, 6, &json!({"delivery":"clipboard"}), result).is_err());
    }
    let mut result = completed();
    result["outputProfile"] = json!("hdr-preserved");
    assert!(
        HostResult::decode(
            "captureDisplay",
            5,
            &json!({"delivery":"clipboard"}),
            result
        )
        .is_err()
    );
}

#[test]
fn validates_capabilities_and_method_version_pairing() {
    let mut caps = json!({"contractVersion":5,"platform":"windows","hostStatus":"available",
        "captureModes":["region","display"],"deliveryTargets":["clipboard","folder"],
        "hdrCapture":"supported","outputProfiles":["srgb-visual-match"]});
    assert!(matches!(
        HostResult::decode("getCapabilities", 5, &json!({}), caps.clone()),
        Ok(HostResult::Capabilities(_))
    ));
    assert!(HostResult::decode("getCapabilities", 6, &json!({}), caps.clone()).is_err());
    caps["hostStatus"] = json!("unavailable");
    assert!(HostResult::decode("getCapabilities", 5, &json!({}), caps.clone()).is_err());
    caps["captureModes"] = json!([]);
    caps["deliveryTargets"] = json!([]);
    caps["unavailableReason"] =
        json!({"code":"host-unavailable","message":"unavailable","retryable":true});
    assert!(HostResult::decode("getCapabilities", 5, &json!({}), caps).is_ok());
    assert!(
        HostResult::decode(
            "captureRegion",
            5,
            &json!({"delivery":"clipboard"}),
            completed()
        )
        .is_err()
    );
    assert!(
        HostResult::decode(
            "captureDisplay",
            5,
            &json!({"delivery":"clipboard"}),
            json!({"status":"released"})
        )
        .is_err()
    );
}

#[test]
fn preserves_partial_delivery_and_failure_details() {
    let mut result = completed();
    result["deliveries"]
        .as_array_mut()
        .unwrap()
        .push(json!({"target":"folder", "status":"failed",
        "failure":{"code":"delivery-failed","message":"Folder is read-only","retryable":true}}));
    let HostResult::Capture(CaptureOutcome::Completed { deliveries, .. }) =
        HostResult::decode("captureRegion", 6, &json!({"delivery":"both"}), result).unwrap()
    else {
        panic!("expected completed result");
    };
    assert!(matches!(
        deliveries[0].outcome,
        DeliveryOutcome::Success { .. }
    ));
    let DeliveryOutcome::Failed { failure } = &deliveries[1].outcome else {
        panic!("expected folder failure");
    };
    assert_eq!(failure.message, "Folder is read-only");
}
