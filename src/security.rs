use std::net::IpAddr;

use thiserror::Error;
use url::{Host, Url};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum UrlSecurityError {
    #[error("实例 URL 无效")]
    Invalid,
    #[error("实例 URL 必须使用 HTTPS；仅 localhost、127.0.0.0/8 或 ::1 可使用 HTTP")]
    InsecureHttp,
    #[error("实例 URL 不能包含用户名、密码、查询参数或片段")]
    UnexpectedComponents,
    #[error("实例 URL 必须指向站点根路径")]
    NonRootPath,
}

/// 验证并规范化 Cloud Memos 实例的 origin。
pub fn validate_instance_url(input: &str) -> Result<Url, UrlSecurityError> {
    let mut url = Url::parse(input).map_err(|_| UrlSecurityError::Invalid)?;
    if url.cannot_be_a_base() || url.host().is_none() {
        return Err(UrlSecurityError::Invalid);
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(UrlSecurityError::UnexpectedComponents);
    }
    if url.path() != "/" && !url.path().is_empty() {
        return Err(UrlSecurityError::NonRootPath);
    }

    match url.scheme() {
        "https" => {}
        "http" if is_loopback(&url) => {}
        "http" => return Err(UrlSecurityError::InsecureHttp),
        _ => return Err(UrlSecurityError::Invalid),
    }

    url.set_path("/");
    Ok(url)
}

fn is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    }
}

#[must_use]
pub fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str().map(str::to_ascii_lowercase)
            == right.host_str().map(str::to_ascii_lowercase)
        && left.port_or_known_default() == right.port_or_known_default()
}

#[must_use]
pub fn is_allowed_transport_url(url: &Url) -> bool {
    match url.scheme() {
        "https" => url.host().is_some(),
        "http" => is_loopback(url),
        _ => false,
    }
}

/// 移除 ANSI/ECMA-48 转义和可能改变终端状态的控制字符。
#[must_use]
pub fn sanitize_terminal(input: &str) -> String {
    let bytes = strip_ansi_escapes::strip(input.as_bytes());
    String::from_utf8_lossy(&bytes)
        .chars()
        .filter(|character| {
            matches!(character, '\n' | '\t')
                || (!character.is_control()
                    && !matches!(u32::from(*character), 0x7f..=0x9f | 0x2028 | 0x2029))
        })
        .collect()
}

#[must_use]
pub fn redact_secret(message: &str, secret: &str) -> String {
    if secret.is_empty() {
        message.to_owned()
    } else {
        message.replace(secret, "[REDACTED]")
    }
}

#[must_use]
pub fn is_loopback_ip(host: &str) -> bool {
    host.parse::<IpAddr>()
        .is_ok_and(|address| address.is_loopback())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_https_and_loopback_http() {
        assert_eq!(
            validate_instance_url("https://memos.example.com")
                .expect("HTTPS should be accepted")
                .as_str(),
            "https://memos.example.com/"
        );
        assert!(validate_instance_url("http://localhost:8787").is_ok());
        assert!(validate_instance_url("http://127.1.2.3:8787/").is_ok());
        assert!(validate_instance_url("http://[::1]:8787").is_ok());
    }

    #[test]
    fn rejects_unsafe_instance_urls() {
        assert_eq!(
            validate_instance_url("http://memos.example.com"),
            Err(UrlSecurityError::InsecureHttp)
        );
        assert_eq!(
            validate_instance_url("https://user:pass@example.com"),
            Err(UrlSecurityError::UnexpectedComponents)
        );
        assert_eq!(
            validate_instance_url("https://example.com/subpath"),
            Err(UrlSecurityError::NonRootPath)
        );
        assert!(validate_instance_url("file:///tmp/memos").is_err());
    }

    #[test]
    fn compares_origins_with_default_ports() {
        let first = Url::parse("https://example.com/").expect("valid URL");
        let second = Url::parse("https://EXAMPLE.com:443/path").expect("valid URL");
        let third = Url::parse("https://example.com:8443/").expect("valid URL");
        assert!(same_origin(&first, &second));
        assert!(!same_origin(&first, &third));
    }

    #[test]
    fn redirect_transport_check_allows_paths_but_not_remote_http() {
        assert!(is_allowed_transport_url(
            &Url::parse("https://example.com/api/v1/session").expect("valid URL")
        ));
        assert!(is_allowed_transport_url(
            &Url::parse("http://127.0.0.1:8787/redirected").expect("valid URL")
        ));
        assert!(!is_allowed_transport_url(
            &Url::parse("http://example.com/api").expect("valid URL")
        ));
    }

    #[test]
    fn strips_terminal_escape_sequences_and_controls() {
        let unsafe_text = "正常\x1b[31m红色\x1b[0m\x07\n下一行\u{009b}31m";
        let safe = sanitize_terminal(unsafe_text);
        assert_eq!(safe, "正常红色\n下一行31m");
        assert!(!safe.contains('\x1b'));
    }

    #[test]
    fn redacts_every_secret_occurrence() {
        assert_eq!(
            redact_secret("bad cm_pat_secret and cm_pat_secret", "cm_pat_secret"),
            "bad [REDACTED] and [REDACTED]"
        );
    }
}
