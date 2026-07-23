use std::time::Duration;

use cloud_memos_cli::{
    ApiClient, App,
    app::{AccessMode, ConfirmAction, Draft, UiMode},
    model::{Memo, MemoAuthor, MemoState, MemoVisibility},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use url::Url;

fn app() -> App {
    let client = ApiClient::with_timeout(
        Url::parse("http://127.0.0.1:9").expect("local URL"),
        "cm_pat_test".to_owned(),
        Duration::from_millis(10),
    )
    .expect("client");
    let mut app = App::new(client, AccessMode::ReadWrite);
    app.items = vec![Memo {
        id: "memo-1".to_owned(),
        content: "正文".to_owned(),
        visibility: MemoVisibility::Private,
        state: MemoState::Active,
        pinned: false,
        version: 1,
        created_at: 1,
        updated_at: 1,
        deleted_at: None,
        author: MemoAuthor {
            id: "user-1".to_owned(),
            name: "用户".to_owned(),
            username: "user".to_owned(),
            image: None,
        },
        tags: Vec::new(),
        attachments: Vec::new(),
    }];
    app
}

#[tokio::test]
async fn visibility_change_requires_confirmation() {
    let mut app = app();
    app.handle_key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE))
        .await;
    assert_eq!(app.mode, UiMode::Confirm);
    assert!(matches!(
        app.confirm.expect("confirmation").action,
        ConfirmAction::ChangeVisibility {
            target: MemoVisibility::Members,
            ..
        }
    ));
}

#[tokio::test]
async fn suspended_conflict_draft_requires_confirmation_before_exit() {
    let mut app = app();
    app.suspended_draft = Some(Draft {
        memo_id: Some("memo-1".to_owned()),
        content: "未保存草稿".to_owned(),
        original: "正文".to_owned(),
        visibility: MemoVisibility::Private,
        version: Some(1),
    });

    app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE))
        .await;
    assert!(!app.should_quit);
    assert_eq!(app.mode, UiMode::Confirm);
    assert!(matches!(
        app.confirm.as_ref().expect("confirmation").action,
        ConfirmAction::DiscardSuspendedDraftAndQuit
    ));

    app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE))
        .await;
    assert!(app.suspended_draft.is_some());
    assert!(!app.should_quit);

    app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE))
        .await;
    app.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE))
        .await;
    assert!(app.should_quit);
    assert!(app.suspended_draft.is_none());
}

#[tokio::test]
async fn read_only_mode_disables_editor_shortcuts() {
    let mut app = app();
    app.access_mode = AccessMode::ReadOnly;
    app.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE))
        .await;
    assert_eq!(app.mode, UiMode::Browse);
    assert!(app.editor.is_none());
    assert!(app.status.contains("只读模式"));
}

#[tokio::test]
async fn editor_never_renders_remote_terminal_controls() {
    let mut app = app();
    app.items[0].content = "正常\x1b[31m红色\x1b[0m".to_owned();
    app.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE))
        .await;
    assert_eq!(app.editor_content().as_deref(), Some("正常红色"));

    app.replace_editor_content("外部\x1b]0;恶意标题\x07正文".to_owned());
    let content = app.editor_content().expect("editor content");
    assert!(!content.contains('\x1b'));
    assert!(!content.contains('\x07'));
}
