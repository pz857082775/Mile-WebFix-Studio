use serde_json::{json, Value};
use url::Url;

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// 按字符截断（对齐 JS slice 的 UTF-16 语义，避免字节切分 panic）
pub fn trunc(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

pub fn is_local_url(u: &Url) -> bool {
    matches!(u.host_str(), Some("localhost") | Some("127.0.0.1") | Some("[::1]") | Some("::1"))
}

/// 授权门禁：非本机目标必须勾选授权（与 JS targetAuthError 一致）
pub fn target_auth_error(url: Option<&str>, authorized: bool) -> Option<String> {
    let url = url?;
    if url.is_empty() {
        return None;
    }
    match Url::parse(url) {
        Err(_) => Some("URL 格式不正确".into()),
        Ok(t) => {
            if !is_local_url(&t) && !authorized {
                Some("请先确认已获得该网站的测试授权".into())
            } else {
                None
            }
        }
    }
}

pub fn err_json(msg: impl Into<String>) -> Value {
    json!({ "error": msg.into() })
}

pub fn join_url(base: &str, path: &str) -> String {
    format!("{}{}", base.trim_end_matches('/'), path)
}

pub fn b64url_encode(s: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(s.as_bytes())
}

pub fn b64url_decode(s: &str) -> Option<String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(s).ok()?;
    String::from_utf8(bytes).ok()
}

/// URL origin（http(s) 才有效），对齐 JS url.origin
pub fn origin_of(u: &Url) -> Option<String> {
    if u.scheme() != "http" && u.scheme() != "https" {
        return None;
    }
    Some(u.origin().ascii_serialization())
}

pub fn home_dir() -> std::path::PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

pub fn config_dir() -> std::path::PathBuf {
    home_dir().join(".webfix-studio")
}