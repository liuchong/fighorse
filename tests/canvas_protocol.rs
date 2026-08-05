use fighorse::canvas::{
    CanvasErrorCode, CanvasOperation, CanvasPlan, CanvasResultStatus, EditorType,
    prepare_plan_assets, validate_plan,
};

#[test]
fn parses_design_fixture_and_serializes_statuses() {
    let plan: CanvasPlan =
        serde_json::from_str(include_str!("fixtures/canvas/design_card_plan.json")).unwrap();

    assert_eq!(plan.expected_editor, Some(EditorType::Figma));
    assert_eq!(plan.operations.len(), 2);
    assert_eq!(CanvasResultStatus::Unknown.to_string(), "unknown");
    assert_eq!(
        CanvasErrorCode::AmbiguousSession.to_string(),
        "ambiguous_session"
    );
}

#[test]
fn rejects_editor_specific_operations_before_plugin_write() {
    let operation = CanvasOperation {
        op: "create_page".to_string(),
        op_id: Some("page".to_string()),
        args: serde_json::json!({ "name": "Should fail" }),
    };
    let plan = CanvasPlan {
        version: 1,
        transaction_id: Some("txn-editor-gate".to_string()),
        session_id: Some("session-figjam".to_string()),
        expected_editor: Some(EditorType::FigJam),
        operations: vec![operation],
        verify: None,
    };

    let err = validate_plan(&plan, EditorType::FigJam).unwrap_err();
    assert_eq!(err.code, CanvasErrorCode::EditorMismatch);
    assert!(err.message.contains("create_page"));
}

#[test]
fn accepts_three_supported_editor_fixtures() {
    let fixtures = [
        (
            include_str!("fixtures/canvas/design_card_plan.json"),
            EditorType::Figma,
        ),
        (
            include_str!("fixtures/canvas/figjam_retro_plan.json"),
            EditorType::FigJam,
        ),
        (
            include_str!("fixtures/canvas/slides_outline_plan.json"),
            EditorType::Slides,
        ),
    ];

    for (fixture, editor) in fixtures {
        let plan: CanvasPlan = serde_json::from_str(fixture).unwrap();
        validate_plan(&plan, editor).unwrap();
    }
}

#[test]
fn prepares_assets_only_from_allowed_roots() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".fighorse")
        .join("exports");
    std::fs::create_dir_all(&root).unwrap();
    let asset = root.join("canvas-test.svg");
    std::fs::write(&asset, r#"<svg xmlns="http://www.w3.org/2000/svg"/>"#).unwrap();

    let mut plan = CanvasPlan {
        version: 1,
        transaction_id: Some("txn-asset".to_string()),
        session_id: Some("session-design".to_string()),
        expected_editor: Some(EditorType::Figma),
        operations: vec![CanvasOperation {
            op: "place_asset".to_string(),
            op_id: Some("asset".to_string()),
            args: serde_json::json!({ "path": asset.to_string_lossy() }),
        }],
        verify: None,
    };
    prepare_plan_assets(&mut plan).unwrap();
    assert_eq!(plan.operations[0].args["mime"], "image/svg+xml");
    assert!(
        plan.operations[0].args["data_base64"]
            .as_str()
            .unwrap()
            .len()
            > 8
    );

    let mut denied = plan.clone();
    denied.operations[0].args = serde_json::json!({ "path": "/etc/hosts" });
    let err = prepare_plan_assets(&mut denied).unwrap_err();
    assert_eq!(err.code, CanvasErrorCode::AssetPathDenied);

    let _ = std::fs::remove_file(asset);
}
