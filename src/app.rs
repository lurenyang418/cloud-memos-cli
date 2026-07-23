use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui_textarea::TextArea;

use crate::{
    api::{ApiClient, ApiError, MemoFilters},
    config::ProfileMode,
    model::{Memo, MemoState, MemoVersion, MemoVisibility, UpdateMemoRequest},
    security::sanitize_terminal,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessMode {
    ReadWrite,
    ReadOnly,
}

impl AccessMode {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ReadWrite => "读写",
            Self::ReadOnly => "只读",
        }
    }
}

impl From<ProfileMode> for AccessMode {
    fn from(value: ProfileMode) -> Self {
        match value {
            ProfileMode::ReadWrite => Self::ReadWrite,
            ProfileMode::ReadOnly => Self::ReadOnly,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Mine,
    Feed,
    Archive,
    Trash,
}

impl View {
    pub const ALL: [Self; 4] = [Self::Mine, Self::Feed, Self::Archive, Self::Trash];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Mine => "我的记录",
            Self::Feed => "成员动态",
            Self::Archive => "归档",
            Self::Trash => "回收站",
        }
    }

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Mine => 0,
            Self::Feed => 1,
            Self::Archive => 2,
            Self::Trash => 3,
        }
    }

    #[must_use]
    pub const fn is_owned(self) -> bool {
        !matches!(self, Self::Feed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiMode {
    Browse,
    Edit,
    Filter,
    Confirm,
    Help,
    History,
    Conflict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventOutcome {
    None,
    OpenExternalEditor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterKind {
    Search,
    Tag,
}

impl FilterKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Search => "搜索",
            Self::Tag => "标签",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Draft {
    pub memo_id: Option<String>,
    pub content: String,
    pub original: String,
    pub visibility: MemoVisibility,
    pub version: Option<u64>,
}

pub struct EditorState {
    pub textarea: TextArea<'static>,
    pub draft: Draft,
}

impl std::fmt::Debug for EditorState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EditorState")
            .field("memo_id", &self.draft.memo_id)
            .field("visibility", &self.draft.visibility)
            .field("version", &self.draft.version)
            .finish_non_exhaustive()
    }
}

impl EditorState {
    fn new(draft: Draft) -> Self {
        let textarea = textarea_with(&draft.content);
        Self { textarea, draft }
    }

    #[must_use]
    pub fn content(&self) -> String {
        self.textarea.lines().join("\n")
    }

    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.content() != self.draft.original
    }

    pub fn replace_content(&mut self, content: String) {
        self.textarea = textarea_with(&content);
    }
}

fn textarea_with(content: &str) -> TextArea<'static> {
    let lines = if content.is_empty() {
        vec![String::new()]
    } else {
        content.lines().map(ToOwned::to_owned).collect()
    };
    TextArea::new(lines)
}

#[derive(Debug)]
pub struct FilterState {
    pub kind: FilterKind,
    pub textarea: TextArea<'static>,
}

#[derive(Debug, Clone)]
pub enum ConfirmAction {
    Delete {
        memo_id: String,
        permanent: bool,
    },
    ChangeState {
        memo_id: String,
        version: u64,
        target: MemoState,
    },
    RestoreVersion {
        memo_id: String,
        target_version: u64,
        current_version: u64,
    },
    ChangeVisibility {
        memo_id: String,
        version: u64,
        target: MemoVisibility,
    },
    OverwriteConflict,
    DiscardDraft,
    DiscardSuspendedDraftAndQuit,
}

#[derive(Debug, Clone)]
pub struct ConfirmState {
    pub prompt: String,
    pub action: ConfirmAction,
}

#[derive(Debug, Clone)]
pub struct ConflictState {
    pub local: Draft,
    pub server: Memo,
}

pub struct App {
    client: ApiClient,
    pub mode: UiMode,
    pub view: View,
    pub items: Vec<Memo>,
    pub selected: usize,
    pub next_cursor: Option<String>,
    pub page_index: usize,
    page_cursors: Vec<Option<String>>,
    pub filters: MemoFilters,
    pub access_mode: AccessMode,
    pub app_name: String,
    pub viewer_name: String,
    pub status: String,
    pub last_error: Option<String>,
    pub should_quit: bool,
    pub show_detail: bool,
    pub editor: Option<EditorState>,
    pub filter: Option<FilterState>,
    pub confirm: Option<ConfirmState>,
    pub history: Vec<MemoVersion>,
    pub history_selected: usize,
    pub conflict: Option<ConflictState>,
    pub suspended_draft: Option<Draft>,
}

impl std::fmt::Debug for App {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("App")
            .field("mode", &self.mode)
            .field("view", &self.view)
            .field("items", &self.items.len())
            .field("selected", &self.selected)
            .field("access_mode", &self.access_mode)
            .finish_non_exhaustive()
    }
}

impl App {
    #[must_use]
    pub fn new(client: ApiClient, access_mode: AccessMode) -> Self {
        Self {
            client,
            mode: UiMode::Browse,
            view: View::Mine,
            items: Vec::new(),
            selected: 0,
            next_cursor: None,
            page_index: 0,
            page_cursors: vec![None],
            filters: MemoFilters {
                limit: Some(20),
                ..MemoFilters::default()
            },
            access_mode,
            app_name: "Cloud Memos".to_owned(),
            viewer_name: String::new(),
            status: "正在连接…".to_owned(),
            last_error: None,
            should_quit: false,
            show_detail: false,
            editor: None,
            filter: None,
            confirm: None,
            history: Vec::new(),
            history_selected: 0,
            conflict: None,
            suspended_draft: None,
        }
    }

    pub async fn bootstrap(&mut self) -> Result<(), ApiError> {
        let session = self.client.session().await?;
        let viewer = session.viewer.ok_or_else(|| ApiError::Response {
            status: reqwest::StatusCode::UNAUTHORIZED,
            code: "UNAUTHENTICATED".to_owned(),
            message: "令牌未返回已登录用户".to_owned(),
            details: None,
        })?;
        self.app_name = sanitize_terminal(&session.app_name);
        self.viewer_name = sanitize_terminal(&viewer.name);
        self.reset_and_refresh().await;
        Ok(())
    }

    #[must_use]
    pub fn selected_memo(&self) -> Option<&Memo> {
        self.items.get(self.selected)
    }

    pub async fn handle_key(&mut self, key: KeyEvent) -> EventOutcome {
        match self.mode {
            UiMode::Browse => self.handle_browse_key(key).await,
            UiMode::Edit => self.handle_edit_key(key).await,
            UiMode::Filter => self.handle_filter_key(key).await,
            UiMode::Confirm => self.handle_confirm_key(key).await,
            UiMode::Help => {
                if matches!(
                    key.code,
                    KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q')
                ) {
                    self.mode = UiMode::Browse;
                }
                EventOutcome::None
            }
            UiMode::History => self.handle_history_key(key).await,
            UiMode::Conflict => self.handle_conflict_key(key).await,
        }
    }

    async fn handle_browse_key(&mut self, key: KeyEvent) -> EventOutcome {
        match key.code {
            KeyCode::Char('q') => {
                if self.suspended_draft.is_some() {
                    self.confirm = Some(ConfirmState {
                        prompt: "仍有保留在内存中的冲突草稿；放弃草稿并退出？".to_owned(),
                        action: ConfirmAction::DiscardSuspendedDraftAndQuit,
                    });
                    self.mode = UiMode::Confirm;
                } else {
                    self.should_quit = true;
                }
            }
            KeyCode::Char('?') => self.mode = UiMode::Help,
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = (self.selected + 1).min(self.items.len().saturating_sub(1));
            }
            KeyCode::Home => self.selected = 0,
            KeyCode::End => self.selected = self.items.len().saturating_sub(1),
            KeyCode::Left | KeyCode::BackTab => self.previous_view().await,
            KeyCode::Right | KeyCode::Tab => self.next_view().await,
            KeyCode::PageDown => self.next_page().await,
            KeyCode::PageUp => self.previous_page().await,
            KeyCode::Enter => self.show_detail = !self.show_detail,
            KeyCode::Esc => self.show_detail = false,
            KeyCode::Char('/') => self.begin_filter(FilterKind::Search),
            KeyCode::Char('t') => self.begin_filter(FilterKind::Tag),
            KeyCode::Char('f') => {
                self.filters.visibility = match self.filters.visibility {
                    None => Some(MemoVisibility::Private),
                    Some(MemoVisibility::Private) => Some(MemoVisibility::Members),
                    Some(MemoVisibility::Members) => Some(MemoVisibility::Public),
                    Some(MemoVisibility::Public) => None,
                };
                self.reset_and_refresh().await;
            }
            KeyCode::Char('r') => self.reload_current().await,
            KeyCode::Char('n') => self.begin_new(),
            KeyCode::Char('e') => self.begin_edit(),
            KeyCode::Char('p') => self.toggle_pin().await,
            KeyCode::Char('v') => self.confirm_visibility(),
            KeyCode::Char('a') => self.confirm_archive(),
            KeyCode::Char('d') => self.confirm_delete(),
            KeyCode::Char('u') => self.restore_from_trash().await,
            KeyCode::Char('h') => self.open_history().await,
            _ => {}
        }
        EventOutcome::None
    }

    async fn handle_edit_key(&mut self, key: KeyEvent) -> EventOutcome {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
            self.save_editor().await;
            return EventOutcome::None;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('e') {
            return EventOutcome::OpenExternalEditor;
        }
        if key.code == KeyCode::Esc {
            let dirty = self.editor.as_ref().is_some_and(EditorState::is_dirty);
            if dirty {
                self.confirm = Some(ConfirmState {
                    prompt: "放弃尚未保存的内容？".to_owned(),
                    action: ConfirmAction::DiscardDraft,
                });
                self.mode = UiMode::Confirm;
            } else {
                self.editor = None;
                self.mode = UiMode::Browse;
            }
            return EventOutcome::None;
        }
        if let Some(editor) = self.editor.as_mut() {
            let _ = editor.textarea.input(key);
        }
        EventOutcome::None
    }

    async fn handle_filter_key(&mut self, key: KeyEvent) -> EventOutcome {
        if key.code == KeyCode::Esc {
            self.filter = None;
            self.mode = UiMode::Browse;
            return EventOutcome::None;
        }
        if key.code == KeyCode::Enter {
            if let Some(filter) = self.filter.take() {
                let value = filter.textarea.lines().join("").trim().to_owned();
                let value = (!value.is_empty()).then_some(value);
                match filter.kind {
                    FilterKind::Search => self.filters.q = value,
                    FilterKind::Tag => self.filters.tag = value,
                }
            }
            self.mode = UiMode::Browse;
            self.reset_and_refresh().await;
            return EventOutcome::None;
        }
        if let Some(filter) = self.filter.as_mut() {
            let _ = filter.textarea.input(key);
        }
        EventOutcome::None
    }

    async fn handle_confirm_key(&mut self, key: KeyEvent) -> EventOutcome {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                if let Some(confirm) = self.confirm.take() {
                    self.execute_confirm(confirm.action).await;
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.confirm = None;
                self.mode = if self.editor.is_some() {
                    UiMode::Edit
                } else if self.conflict.is_some() {
                    UiMode::Conflict
                } else {
                    UiMode::Browse
                };
            }
            _ => {}
        }
        EventOutcome::None
    }

    async fn handle_history_key(&mut self, key: KeyEvent) -> EventOutcome {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.history.clear();
                self.mode = UiMode::Browse;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.history_selected = self.history_selected.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.history_selected =
                    (self.history_selected + 1).min(self.history.len().saturating_sub(1));
            }
            KeyCode::Enter | KeyCode::Char('r') => {
                let memo = self.selected_memo().cloned();
                let version = self.history.get(self.history_selected).cloned();
                if let (Some(memo), Some(version)) = (memo, version)
                    && self.ensure_write()
                {
                    self.confirm = Some(ConfirmState {
                        prompt: format!("恢复到历史版本 v{}？", version.version),
                        action: ConfirmAction::RestoreVersion {
                            memo_id: memo.id,
                            target_version: version.version,
                            current_version: memo.version,
                        },
                    });
                    self.mode = UiMode::Confirm;
                }
            }
            _ => {}
        }
        EventOutcome::None
    }

    async fn handle_conflict_key(&mut self, key: KeyEvent) -> EventOutcome {
        match key.code {
            KeyCode::Char('m') => {
                if let Some(conflict) = self.conflict.take() {
                    let mut draft = conflict.local;
                    draft.version = Some(conflict.server.version);
                    draft.original = sanitize_terminal(&conflict.server.content);
                    self.editor = Some(EditorState::new(draft));
                    self.mode = UiMode::Edit;
                }
            }
            KeyCode::Char('o') => {
                self.confirm = Some(ConfirmState {
                    prompt: "使用服务端最新版本号，以本地草稿覆盖服务端正文？".to_owned(),
                    action: ConfirmAction::OverwriteConflict,
                });
                self.mode = UiMode::Confirm;
            }
            KeyCode::Esc | KeyCode::Char('c') => {
                if let Some(conflict) = self.conflict.take() {
                    self.suspended_draft = Some(conflict.local);
                }
                self.status = "草稿仍保留在内存中；按 e 可继续编辑".to_owned();
                self.mode = UiMode::Browse;
            }
            _ => {}
        }
        EventOutcome::None
    }

    pub fn editor_content(&self) -> Option<String> {
        self.editor.as_ref().map(EditorState::content)
    }

    pub fn replace_editor_content(&mut self, content: String) {
        if let Some(editor) = self.editor.as_mut() {
            editor.replace_content(sanitize_terminal(&content));
            self.status = "已从外部编辑器载入内容，按 Ctrl+S 保存".to_owned();
        }
    }

    pub fn set_status_error(&mut self, message: impl Into<String>) {
        self.last_error = Some(sanitize_terminal(&message.into()));
    }

    fn begin_filter(&mut self, kind: FilterKind) {
        let existing = match kind {
            FilterKind::Search => self.filters.q.as_deref(),
            FilterKind::Tag => self.filters.tag.as_deref(),
        }
        .unwrap_or_default();
        self.filter = Some(FilterState {
            kind,
            textarea: textarea_with(existing),
        });
        self.mode = UiMode::Filter;
    }

    fn begin_new(&mut self) {
        if self.view != View::Mine || !self.ensure_write() {
            if self.view != View::Mine {
                self.status = "请先切换到“我的记录”再新建".to_owned();
            }
            return;
        }
        self.editor = Some(EditorState::new(Draft {
            memo_id: None,
            content: String::new(),
            original: String::new(),
            visibility: MemoVisibility::Private,
            version: None,
        }));
        self.mode = UiMode::Edit;
    }

    fn begin_edit(&mut self) {
        if !self.view.is_owned() || self.view == View::Trash {
            self.status = "当前视图不能编辑".to_owned();
            return;
        }
        if !self.ensure_write() {
            return;
        }
        let Some(memo) = self.selected_memo().cloned() else {
            return;
        };
        let resume_suspended = self
            .suspended_draft
            .as_ref()
            .is_some_and(|draft| draft.memo_id.as_deref() == Some(&memo.id));
        if resume_suspended {
            let draft = self
                .suspended_draft
                .take()
                .expect("suspended draft was just checked");
            self.editor = Some(EditorState::new(draft));
        } else {
            let safe_content = sanitize_terminal(&memo.content);
            self.editor = Some(EditorState::new(Draft {
                memo_id: Some(memo.id),
                content: safe_content.clone(),
                original: safe_content,
                visibility: memo.visibility,
                version: Some(memo.version),
            }));
        }
        self.mode = UiMode::Edit;
    }

    async fn save_editor(&mut self) {
        let Some(editor) = self.editor.as_ref() else {
            return;
        };
        let content = editor.content();
        if content.trim().is_empty() {
            self.status = "正文不能为空".to_owned();
            return;
        }
        if content.len() > 100_000 {
            self.status = "正文不能超过 100,000 字节".to_owned();
            return;
        }
        let draft = Draft {
            content: content.trim().to_owned(),
            ..editor.draft.clone()
        };
        let result = if let (Some(id), Some(version)) = (&draft.memo_id, draft.version) {
            self.client
                .update_memo(
                    id,
                    &UpdateMemoRequest {
                        content: Some(draft.content.clone()),
                        version,
                        ..UpdateMemoRequest::default()
                    },
                )
                .await
        } else {
            self.client
                .create_memo(&draft.content, draft.visibility)
                .await
        };

        match result {
            Ok(_) => {
                self.editor = None;
                self.suspended_draft = None;
                self.mode = UiMode::Browse;
                self.status = "已保存".to_owned();
                self.reset_and_refresh().await;
            }
            Err(error) if error.is_version_conflict() && draft.memo_id.is_some() => {
                let id = draft.memo_id.as_deref().unwrap_or_default();
                match self.client.get_memo(id).await {
                    Ok(server) => {
                        self.editor = None;
                        self.conflict = Some(ConflictState {
                            local: draft,
                            server,
                        });
                        self.mode = UiMode::Conflict;
                        self.status = "检测到版本冲突；本地草稿已保留".to_owned();
                    }
                    Err(refresh_error) => {
                        self.suspended_draft = Some(draft);
                        self.editor = None;
                        self.mode = UiMode::Browse;
                        self.record_error(refresh_error);
                    }
                }
            }
            Err(error) => self.record_write_error(error),
        }
    }

    async fn overwrite_conflict(&mut self) {
        if !self.ensure_write() {
            return;
        }
        let Some(conflict) = self.conflict.clone() else {
            return;
        };
        let Some(id) = conflict.local.memo_id.as_deref() else {
            return;
        };
        let result = self
            .client
            .update_memo(
                id,
                &UpdateMemoRequest {
                    content: Some(conflict.local.content.clone()),
                    version: conflict.server.version,
                    ..UpdateMemoRequest::default()
                },
            )
            .await;
        match result {
            Ok(_) => {
                self.conflict = None;
                self.mode = UiMode::Browse;
                self.status = "已使用最新版本号覆盖保存".to_owned();
                self.reset_and_refresh().await;
            }
            Err(error) if error.is_version_conflict() => match self.client.get_memo(id).await {
                Ok(server) => {
                    if let Some(current) = self.conflict.as_mut() {
                        current.server = server;
                    }
                    self.status = "保存期间再次发生冲突；草稿仍保留".to_owned();
                }
                Err(refresh_error) => self.record_error(refresh_error),
            },
            Err(error) => self.record_write_error(error),
        }
    }

    async fn toggle_pin(&mut self) {
        if !self.view.is_owned() || self.view == View::Trash || !self.ensure_write() {
            return;
        }
        let Some(memo) = self.selected_memo().cloned() else {
            return;
        };
        let result = self
            .client
            .update_memo(
                &memo.id,
                &UpdateMemoRequest {
                    pinned: Some(!memo.pinned),
                    version: memo.version,
                    ..UpdateMemoRequest::default()
                },
            )
            .await;
        self.finish_write(result, "已切换置顶状态").await;
    }

    fn confirm_visibility(&mut self) {
        if !self.view.is_owned() || self.view == View::Trash || !self.ensure_write() {
            return;
        }
        let Some(memo) = self.selected_memo().cloned() else {
            return;
        };
        let next = memo.visibility.next();
        self.confirm = Some(ConfirmState {
            prompt: format!("将这条记录的可见性切换为“{}”？", next.label()),
            action: ConfirmAction::ChangeVisibility {
                memo_id: memo.id,
                version: memo.version,
                target: next,
            },
        });
        self.mode = UiMode::Confirm;
    }

    fn confirm_archive(&mut self) {
        if !matches!(self.view, View::Mine | View::Archive) || !self.ensure_write() {
            return;
        }
        let Some(memo) = self.selected_memo().cloned() else {
            return;
        };
        let target = if memo.state == MemoState::Archived {
            MemoState::Active
        } else {
            MemoState::Archived
        };
        let prompt = if target == MemoState::Archived {
            "归档这条记录？"
        } else {
            "将这条记录移回“我的记录”？"
        };
        self.confirm = Some(ConfirmState {
            prompt: prompt.to_owned(),
            action: ConfirmAction::ChangeState {
                memo_id: memo.id,
                version: memo.version,
                target,
            },
        });
        self.mode = UiMode::Confirm;
    }

    fn confirm_delete(&mut self) {
        if !matches!(self.view, View::Mine | View::Archive | View::Trash) || !self.ensure_write() {
            return;
        }
        let Some(memo) = self.selected_memo().cloned() else {
            return;
        };
        let permanent = self.view == View::Trash;
        self.confirm = Some(ConfirmState {
            prompt: if permanent {
                "永久删除这条记录？此操作不可恢复。".to_owned()
            } else {
                "将这条记录移入回收站？".to_owned()
            },
            action: ConfirmAction::Delete {
                memo_id: memo.id,
                permanent,
            },
        });
        self.mode = UiMode::Confirm;
    }

    async fn restore_from_trash(&mut self) {
        if self.view != View::Trash || !self.ensure_write() {
            return;
        }
        let Some(memo) = self.selected_memo().cloned() else {
            return;
        };
        let result = self.client.restore_memo(&memo.id).await;
        self.finish_write(result, "已从回收站恢复").await;
    }

    async fn open_history(&mut self) {
        if !matches!(self.view, View::Mine | View::Archive) {
            self.status = "当前视图不提供历史版本".to_owned();
            return;
        }
        let Some(memo) = self.selected_memo().cloned() else {
            return;
        };
        match self.client.versions(&memo.id).await {
            Ok(versions) => {
                self.history = versions;
                self.history_selected = 0;
                self.mode = UiMode::History;
            }
            Err(error) => self.record_error(error),
        }
    }

    async fn execute_confirm(&mut self, action: ConfirmAction) {
        match action {
            ConfirmAction::DiscardDraft => {
                self.editor = None;
                self.mode = UiMode::Browse;
                self.status = "已放弃未保存内容".to_owned();
            }
            ConfirmAction::DiscardSuspendedDraftAndQuit => {
                self.suspended_draft = None;
                self.should_quit = true;
                self.mode = UiMode::Browse;
            }
            ConfirmAction::OverwriteConflict => {
                self.mode = UiMode::Conflict;
                self.overwrite_conflict().await;
            }
            ConfirmAction::Delete { memo_id, permanent } => {
                self.mode = UiMode::Browse;
                let result = if permanent {
                    self.client.permanently_delete_memo(&memo_id).await
                } else {
                    self.client.delete_memo(&memo_id).await
                };
                match result {
                    Ok(()) => {
                        self.status = if permanent {
                            "已永久删除".to_owned()
                        } else {
                            "已移入回收站".to_owned()
                        };
                        self.reset_and_refresh().await;
                    }
                    Err(error) => self.record_write_error(error),
                }
            }
            ConfirmAction::ChangeState {
                memo_id,
                version,
                target,
            } => {
                self.mode = UiMode::Browse;
                let result = self
                    .client
                    .update_memo(
                        &memo_id,
                        &UpdateMemoRequest {
                            state: Some(target),
                            version,
                            ..UpdateMemoRequest::default()
                        },
                    )
                    .await;
                self.finish_write(result, "记录状态已更新").await;
            }
            ConfirmAction::RestoreVersion {
                memo_id,
                target_version,
                current_version,
            } => {
                self.mode = UiMode::Browse;
                self.history.clear();
                let result = self
                    .client
                    .restore_version(&memo_id, target_version, current_version)
                    .await;
                self.finish_write(result, "历史版本已恢复").await;
            }
            ConfirmAction::ChangeVisibility {
                memo_id,
                version,
                target,
            } => {
                self.mode = UiMode::Browse;
                let result = self
                    .client
                    .update_memo(
                        &memo_id,
                        &UpdateMemoRequest {
                            visibility: Some(target),
                            version,
                            ..UpdateMemoRequest::default()
                        },
                    )
                    .await;
                self.finish_write(result, &format!("可见性已切换为“{}”", target.label()))
                    .await;
            }
        }
    }

    async fn finish_write(&mut self, result: Result<Memo, ApiError>, success: &str) {
        match result {
            Ok(_) => {
                self.status = success.to_owned();
                self.reset_and_refresh().await;
            }
            Err(error) => self.record_write_error(error),
        }
    }

    fn ensure_write(&mut self) -> bool {
        if self.access_mode == AccessMode::ReadOnly {
            self.status = "当前运行是只读模式，写操作已禁用".to_owned();
            false
        } else {
            true
        }
    }

    fn record_write_error(&mut self, error: ApiError) {
        if error.is_insufficient_scope() {
            self.access_mode = AccessMode::ReadOnly;
            self.status = "服务器拒绝写入权限；当前运行已自动降级为只读".to_owned();
        }
        self.record_error(error);
    }

    fn record_error(&mut self, error: ApiError) {
        self.last_error = Some(sanitize_terminal(&error.to_string()));
        if self.status.is_empty() || !error.is_insufficient_scope() {
            self.status = "请求失败；按 r 重试".to_owned();
        }
    }

    async fn previous_view(&mut self) {
        let index = (self.view.index() + View::ALL.len() - 1) % View::ALL.len();
        self.view = View::ALL[index];
        self.show_detail = false;
        self.reset_and_refresh().await;
    }

    async fn next_view(&mut self) {
        let index = (self.view.index() + 1) % View::ALL.len();
        self.view = View::ALL[index];
        self.show_detail = false;
        self.reset_and_refresh().await;
    }

    async fn reset_and_refresh(&mut self) {
        self.page_cursors = vec![None];
        self.page_index = 0;
        self.load_cursor(None).await;
    }

    async fn reload_current(&mut self) {
        let cursor = self
            .page_cursors
            .get(self.page_index)
            .cloned()
            .unwrap_or(None);
        self.load_cursor(cursor).await;
    }

    async fn next_page(&mut self) {
        let Some(cursor) = self.next_cursor.clone() else {
            self.status = "已经是最后一页".to_owned();
            return;
        };
        self.page_cursors.truncate(self.page_index + 1);
        self.page_cursors.push(Some(cursor.clone()));
        self.page_index += 1;
        self.load_cursor(Some(cursor)).await;
    }

    async fn previous_page(&mut self) {
        if self.page_index == 0 {
            self.status = "已经是第一页".to_owned();
            return;
        }
        self.page_index -= 1;
        let cursor = self.page_cursors[self.page_index].clone();
        self.load_cursor(cursor).await;
    }

    async fn load_cursor(&mut self, cursor: Option<String>) {
        let mut filters = self.filters.clone();
        filters.cursor = cursor;
        filters.state = match self.view {
            View::Archive => Some(MemoState::Archived),
            View::Mine => Some(MemoState::Active),
            View::Feed | View::Trash => None,
        };
        filters.deleted = (self.view == View::Trash).then_some(true);

        let result = if self.view == View::Feed {
            self.client.list_feed(&filters).await
        } else {
            self.client.list_memos(&filters).await
        };
        match result {
            Ok(page) => {
                self.items = page.items;
                self.next_cursor = page.next_cursor;
                self.selected = self.selected.min(self.items.len().saturating_sub(1));
                self.last_error = None;
                self.status = format!(
                    "{} · 第 {} 页 · {} 条 · {}",
                    self.view.label(),
                    self.page_index + 1,
                    self.items.len(),
                    self.access_mode.label()
                );
            }
            Err(error) => self.record_error(error),
        }
    }
}
