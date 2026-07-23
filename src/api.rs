use std::{sync::Arc, time::Duration};

use reqwest::{
    Method, StatusCode,
    header::{AUTHORIZATION, HeaderValue},
    redirect,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use thiserror::Error;
use url::Url;

use crate::{
    model::{
        ApiErrorEnvelope, CreateMemoRequest, CursorPage, Memo, MemoState, MemoVersion,
        MemoVisibility, SessionResponse, UpdateMemoRequest, VersionPage,
    },
    security::{is_allowed_transport_url, redact_secret, same_origin, validate_instance_url},
};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_REDIRECTS: usize = 5;

#[derive(Debug, Error, Clone)]
pub enum ApiError {
    #[error("网络请求失败：{0}")]
    Transport(String),
    #[error("Cloud Memos 请求失败 ({status} {code})：{message}")]
    Response {
        status: StatusCode,
        code: String,
        message: String,
        details: Option<Value>,
    },
    #[error("服务器返回了无法解析的响应：{0}")]
    InvalidResponse(String),
}

impl ApiError {
    #[must_use]
    pub fn code(&self) -> Option<&str> {
        match self {
            Self::Response { code, .. } => Some(code),
            Self::Transport(_) | Self::InvalidResponse(_) => None,
        }
    }

    #[must_use]
    pub fn is_insufficient_scope(&self) -> bool {
        self.code() == Some("INSUFFICIENT_SCOPE")
    }

    #[must_use]
    pub fn is_version_conflict(&self) -> bool {
        self.code() == Some("VERSION_CONFLICT")
    }

    #[must_use]
    pub fn current_version(&self) -> Option<u64> {
        let Self::Response { details, .. } = self else {
            return None;
        };
        details
            .as_ref()
            .and_then(|value| value.get("currentVersion"))
            .and_then(Value::as_u64)
    }
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct MemoFilters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<MemoVisibility>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<MemoState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted: Option<bool>,
}

#[derive(Clone)]
pub struct ApiClient {
    base_url: Url,
    client: reqwest::Client,
    token: Arc<str>,
}

impl std::fmt::Debug for ApiClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ApiClient")
            .field("base_url", &self.base_url)
            .field("token", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl ApiClient {
    pub fn new(base_url: Url, token: String) -> Result<Self, ApiError> {
        Self::with_timeout(base_url, token, DEFAULT_TIMEOUT)
    }

    pub fn with_timeout(base_url: Url, token: String, timeout: Duration) -> Result<Self, ApiError> {
        let normalized = validate_instance_url(base_url.as_str())
            .map_err(|error| ApiError::Transport(error.to_string()))?;
        let redirect_origin = normalized.clone();
        let policy = redirect::Policy::custom(move |attempt| {
            if attempt.previous().len() >= MAX_REDIRECTS {
                return attempt.error("重定向次数过多");
            }
            if !is_allowed_transport_url(attempt.url())
                || !same_origin(&redirect_origin, attempt.url())
            {
                return attempt.error("已阻止跨源或不安全重定向");
            }
            attempt.follow()
        });
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .redirect(policy)
            .user_agent(concat!("cloud-memos-cli/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| ApiError::Transport(redact_secret(&error.to_string(), &token)))?;
        Ok(Self {
            base_url: normalized,
            client,
            token: Arc::from(token),
        })
    }

    #[must_use]
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    fn endpoint(&self, path: &str) -> Result<Url, ApiError> {
        self.base_url.join(path).map_err(|error| {
            ApiError::Transport(redact_secret(&error.to_string(), self.token.as_ref()))
        })
    }

    fn request(&self, method: Method, path: &str) -> Result<reqwest::RequestBuilder, ApiError> {
        let mut authorization = HeaderValue::from_str(&format!("Bearer {}", self.token))
            .map_err(|_| ApiError::Transport("API 令牌无法编码为 HTTP 请求头".to_owned()))?;
        authorization.set_sensitive(true);
        Ok(self
            .client
            .request(method, self.endpoint(path)?)
            .header(AUTHORIZATION, authorization))
    }

    async fn send_json<T: DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<T, ApiError> {
        let response = request.send().await.map_err(|error| {
            ApiError::Transport(redact_secret(&error.to_string(), self.token.as_ref()))
        })?;
        let status = response.status();
        if !status.is_success() {
            return Err(self.response_error(status, response).await);
        }
        response.json::<T>().await.map_err(|error| {
            ApiError::InvalidResponse(redact_secret(&error.to_string(), self.token.as_ref()))
        })
    }

    async fn send_empty(&self, request: reqwest::RequestBuilder) -> Result<(), ApiError> {
        let response = request.send().await.map_err(|error| {
            ApiError::Transport(redact_secret(&error.to_string(), self.token.as_ref()))
        })?;
        let status = response.status();
        if !status.is_success() {
            return Err(self.response_error(status, response).await);
        }
        Ok(())
    }

    async fn response_error(&self, status: StatusCode, response: reqwest::Response) -> ApiError {
        match response.json::<ApiErrorEnvelope>().await {
            Ok(envelope) => ApiError::Response {
                status,
                code: redact_secret(&envelope.error.code, self.token.as_ref()),
                message: redact_secret(&envelope.error.message, self.token.as_ref()),
                details: envelope.error.details.map(|details| {
                    let encoded = redact_secret(&details.to_string(), self.token.as_ref());
                    serde_json::from_str(&encoded).unwrap_or(Value::String(encoded))
                }),
            },
            Err(_) => ApiError::Response {
                status,
                code: "REQUEST_FAILED".to_owned(),
                message: format!("请求失败（HTTP {status}）"),
                details: None,
            },
        }
    }

    pub async fn session(&self) -> Result<SessionResponse, ApiError> {
        let request = self.request(Method::GET, "api/v1/session")?;
        self.send_json(request).await
    }

    pub async fn list_memos(&self, filters: &MemoFilters) -> Result<CursorPage<Memo>, ApiError> {
        let request = self.request(Method::GET, "api/v1/memos")?.query(filters);
        self.send_json(request).await
    }

    pub async fn list_feed(&self, filters: &MemoFilters) -> Result<CursorPage<Memo>, ApiError> {
        let request = self.request(Method::GET, "api/v1/feed")?.query(filters);
        self.send_json(request).await
    }

    pub async fn get_memo(&self, id: &str) -> Result<Memo, ApiError> {
        let request = self.request(Method::GET, &format!("api/v1/memos/{id}"))?;
        self.send_json(request).await
    }

    pub async fn create_memo(
        &self,
        content: &str,
        visibility: MemoVisibility,
    ) -> Result<Memo, ApiError> {
        let body = CreateMemoRequest {
            content,
            visibility,
            attachment_ids: Vec::new(),
        };
        let request = self.request(Method::POST, "api/v1/memos")?.json(&body);
        self.send_json(request).await
    }

    pub async fn update_memo(&self, id: &str, body: &UpdateMemoRequest) -> Result<Memo, ApiError> {
        let request = self
            .request(Method::PATCH, &format!("api/v1/memos/{id}"))?
            .json(body);
        self.send_json(request).await
    }

    pub async fn delete_memo(&self, id: &str) -> Result<(), ApiError> {
        let request = self.request(Method::DELETE, &format!("api/v1/memos/{id}"))?;
        self.send_empty(request).await
    }

    pub async fn restore_memo(&self, id: &str) -> Result<Memo, ApiError> {
        let request = self.request(Method::POST, &format!("api/v1/memos/{id}/restore"))?;
        self.send_json(request).await
    }

    pub async fn permanently_delete_memo(&self, id: &str) -> Result<(), ApiError> {
        let request = self.request(Method::DELETE, &format!("api/v1/memos/{id}/permanent"))?;
        self.send_empty(request).await
    }

    pub async fn versions(&self, id: &str) -> Result<Vec<MemoVersion>, ApiError> {
        let request = self.request(Method::GET, &format!("api/v1/memos/{id}/versions"))?;
        let page: VersionPage = self.send_json(request).await?;
        Ok(page.items)
    }

    pub async fn restore_version(
        &self,
        id: &str,
        target_version: u64,
        current_version: u64,
    ) -> Result<Memo, ApiError> {
        #[derive(Serialize)]
        struct RestoreVersion {
            version: u64,
        }
        let request = self
            .request(
                Method::POST,
                &format!("api/v1/memos/{id}/versions/{target_version}/restore"),
            )?
            .json(&RestoreVersion {
                version: current_version,
            });
        self.send_json(request).await
    }
}
