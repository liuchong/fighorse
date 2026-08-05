use std::time::Duration;

use fighorse::canvas::{
    CanvasErrorCode, CanvasManager, CanvasPlan, CanvasSession, CanvasSessionSummary, EditorType,
};

fn session(id: &str, editor: EditorType) -> CanvasSession {
    CanvasSession {
        session_id: id.to_string(),
        plugin_version: "0.1.0".to_string(),
        editor_type: editor,
        document_name: "Redacted document".to_string(),
        current_page: Some("Page 1".to_string()),
        selection_count: 0,
        capabilities: vec!["inspect".to_string(), "apply".to_string()],
        connected_at_ms: 1,
        last_heartbeat_ms: 1,
    }
}

#[tokio::test]
async fn pairing_codes_are_single_use() {
    let manager = CanvasManager::new_for_tests();
    let pairing = manager
        .create_pairing(Duration::from_secs(300))
        .await
        .unwrap();
    assert!(pairing.code.starts_with("pair-"));
    assert!(pairing.code.len() >= 37);

    let first = manager
        .redeem_pairing(&pairing.code, session("session-a", EditorType::Figma))
        .await;
    assert!(first.is_ok());

    let second = manager
        .redeem_pairing(&pairing.code, session("session-b", EditorType::Figma))
        .await
        .unwrap_err();
    assert_eq!(second.code, CanvasErrorCode::PairingNotFound);
}

#[tokio::test]
async fn write_without_session_is_rejected_when_ambiguous() {
    let manager = CanvasManager::new_for_tests();
    manager
        .register_test_session(CanvasSessionSummary::from(session(
            "session-a",
            EditorType::Figma,
        )))
        .await;
    manager
        .register_test_session(CanvasSessionSummary::from(session(
            "session-b",
            EditorType::Figma,
        )))
        .await;

    let plan = CanvasPlan {
        version: 1,
        transaction_id: Some("txn-ambiguous".to_string()),
        session_id: None,
        expected_editor: Some(EditorType::Figma),
        operations: vec![],
        verify: None,
    };

    let err = manager.resolve_session_for_plan(&plan).await.unwrap_err();
    assert_eq!(err.code, CanvasErrorCode::AmbiguousSession);
}
