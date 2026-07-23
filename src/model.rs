use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MemoVisibility {
    #[default]
    Private,
    Members,
    Public,
}

impl MemoVisibility {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Private => "仅自己",
            Self::Members => "成员",
            Self::Public => "公开",
        }
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Private => Self::Members,
            Self::Members => Self::Public,
            Self::Public => Self::Private,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MemoState {
    Active,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Viewer {
    pub id: String,
    pub name: String,
    pub email: String,
    pub username: String,
    pub role: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicContact {
    pub label: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResponse {
    pub viewer: Option<Viewer>,
    pub setup_required: bool,
    pub app_name: String,
    pub public_contact: Option<PublicContact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: String,
    pub filename: String,
    pub content_type: String,
    pub size: u64,
    pub status: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoAuthor {
    pub id: String,
    pub name: String,
    pub username: String,
    pub image: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Memo {
    pub id: String,
    pub content: String,
    pub visibility: MemoVisibility,
    pub state: MemoState,
    pub pinned: bool,
    pub version: u64,
    pub created_at: u64,
    pub updated_at: u64,
    pub deleted_at: Option<u64>,
    pub author: MemoAuthor,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoVersion {
    pub id: String,
    pub memo_id: String,
    pub content: String,
    pub visibility: MemoVisibility,
    pub state: MemoState,
    pub pinned: bool,
    pub version: u64,
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorPage<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ApiErrorEnvelope {
    pub error: ApiErrorBody,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ApiErrorBody {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateMemoRequest<'a> {
    pub content: &'a str,
    pub visibility: MemoVisibility,
    pub attachment_ids: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMemoRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<MemoVisibility>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<MemoState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
    pub version: u64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct VersionPage {
    pub items: Vec<MemoVersion>,
}
