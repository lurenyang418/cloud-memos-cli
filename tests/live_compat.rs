use std::{
    env,
    time::{SystemTime, UNIX_EPOCH},
};

use cloud_memos_cli::{
    ApiClient, MemoFilters,
    model::{MemoVisibility, UpdateMemoRequest},
    security::{sanitize_terminal, validate_instance_url},
};

#[tokio::test]
#[ignore = "requires CLOUD_MEMOS_TEST_URL and CLOUD_MEMOS_TEST_TOKEN"]
async fn live_cloud_memos_v0_3_0_compatibility() {
    let url = env::var("CLOUD_MEMOS_TEST_URL")
        .expect("set CLOUD_MEMOS_TEST_URL to a local or staging instance");
    let token = env::var("CLOUD_MEMOS_TEST_TOKEN")
        .expect("set CLOUD_MEMOS_TEST_TOKEN; the test never prints it");
    let client = ApiClient::new(
        validate_instance_url(&url).expect("test URL must satisfy production URL policy"),
        token,
    )
    .expect("create API client");

    let session = client.session().await.expect("GET /api/v1/session");
    let viewer = session.viewer.expect("PAT must return a viewer");
    assert_eq!(viewer.status, "ACTIVE");
    let page = client
        .list_memos(&MemoFilters {
            limit: Some(1),
            ..MemoFilters::default()
        })
        .await
        .expect("GET /api/v1/memos");
    println!(
        "read compatibility passed for app {:?}; first page contains {} item(s)",
        sanitize_terminal(&session.app_name),
        page.items.len()
    );

    if env::var("CLOUD_MEMOS_TEST_WRITE").as_deref() != Ok("1") {
        println!("write probe skipped; set CLOUD_MEMOS_TEST_WRITE=1 only on dedicated staging");
        return;
    }

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_secs();
    let created = client
        .create_memo(
            &format!("cloud-memos-cli compatibility probe {nonce}"),
            MemoVisibility::Private,
        )
        .await
        .expect("create probe memo");
    let updated = client
        .update_memo(
            &created.id,
            &UpdateMemoRequest {
                content: Some(format!(
                    "cloud-memos-cli compatibility probe {nonce} updated"
                )),
                version: created.version,
                ..UpdateMemoRequest::default()
            },
        )
        .await
        .expect("update probe memo with optimistic version");
    assert_eq!(updated.version, created.version + 1);
    client
        .delete_memo(&created.id)
        .await
        .expect("move probe memo to trash");
    client
        .permanently_delete_memo(&created.id)
        .await
        .expect("permanently delete probe memo");
    println!("write compatibility probe passed and its memo was deleted");
}
