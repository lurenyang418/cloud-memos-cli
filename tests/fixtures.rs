use std::fs;

use cloud_memos_cli::model::{CursorPage, Memo, SessionResponse};
use serde_json::Value;

const FIXTURE_DIR: &str = "fixtures/cloud-memos-v0.3.0";

#[test]
fn baseline_metadata_is_pinned_to_exact_upstream_commit() {
    let metadata: Value = serde_json::from_str(
        &fs::read_to_string(format!("{FIXTURE_DIR}/metadata.json")).expect("metadata fixture"),
    )
    .expect("valid metadata JSON");
    assert_eq!(metadata["upstream"]["tag"], "v0.3.0");
    assert_eq!(
        metadata["upstream"]["commit"],
        "0864c2327135779e4ac5baf0a407c082a35a1f42"
    );
    assert_eq!(metadata["upstream"]["apiBasePath"], "/api/v1");
}

#[test]
fn sanitized_upstream_responses_match_client_types() {
    let session: SessionResponse = serde_json::from_str(
        &fs::read_to_string(format!("{FIXTURE_DIR}/session.json")).expect("session fixture"),
    )
    .expect("session type");
    assert_eq!(
        session.viewer.expect("viewer").email,
        "redacted@example.invalid"
    );

    let page: CursorPage<Memo> = serde_json::from_str(
        &fs::read_to_string(format!("{FIXTURE_DIR}/memos-page.json")).expect("memo page fixture"),
    )
    .expect("memo page type");
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].attachments.len(), 1);
    assert!(page.next_cursor.is_some());
}

#[test]
fn conflict_fixture_preserves_expected_error_code_and_version_shape() {
    let conflict: Value = serde_json::from_str(
        &fs::read_to_string(format!("{FIXTURE_DIR}/version-conflict.json"))
            .expect("conflict fixture"),
    )
    .expect("conflict JSON");
    assert_eq!(conflict["error"]["code"], "VERSION_CONFLICT");
    assert_eq!(conflict["error"]["details"]["currentVersion"], 3);
}
