// 软件更新：GitHub Releases 检测 + 可配置 manifest 覆盖渠道 + 下载到本地（手动安装，不自动执行）
// 配置持久化于系统应用数据目录 update.json（重装 / 升级不丢失）。
use crate::util::{config_dir, now_ms, VERSION};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Duration;

/// 默认发布仓库（GitHub Releases）
pub const REPO: &str = "pz857082775/Mile-WebFix-Studio";
const MAX_DOWNLOAD_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone)]
pub struct UpdateConfig {
    /// 覆盖渠道：自定义 manifest JSON 地址（留空则用 GitHub Releases）
    pub manifest_url: String,
    /// 启动时静默检测更新
    pub auto_check: bool,
}

fn config_file() -> PathBuf { config_dir().join("update.json") }

pub fn load_config() -> UpdateConfig {
    let v = std::fs::read_to_string(config_file()).ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .unwrap_or(Value::Null);
    UpdateConfig {
        manifest_url: v["manifestUrl"].as_str().unwrap_or("").to_string(),
        auto_check: v["autoCheck"].as_bool().unwrap_or(true),
    }
}

pub fn save_config(manifest_url: &str, auto_check: bool) -> UpdateConfig {
    let c = UpdateConfig { manifest_url: manifest_url.trim().to_string(), auto_check };
    let _ = std::fs::create_dir_all(config_dir());
    let _ = std::fs::write(config_file(), serde_json::to_string_pretty(&json!({
        "manifestUrl": c.manifest_url, "autoCheck": c.auto_check, "updatedAt": now_ms(),
    })).unwrap_or_default());
    c
}

/// 解析语义化版本为可比较三元组（忽略 v 前缀与 -pre/+build 后缀）
fn parse_ver(s: &str) -> (u64, u64, u64) {
    let t = s.trim().trim_start_matches(['v', 'V']);
    let core = t.split(['-', '+']).next().unwrap_or(t);
    let mut it = core.split('.');
    let a = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let b = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let c = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    (a, b, c)
}

pub fn newer(latest: &str, current: &str) -> bool { parse_ver(latest) > parse_ver(current) }

/// 下载文件名净化：仅保留基名中的安全字符，杜绝路径穿越
fn safe_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = base.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' }).collect();
    let trimmed = cleaned.trim_matches('.');
    if trimmed.is_empty() { "webfix-update.bin".to_string() } else { trimmed.to_string() }
}

fn is_http(u: &str) -> bool { u.starts_with("http://") || u.starts_with("https://") }

/// 统一解析 release / manifest JSON（兼容 GitHub Releases 与简单 manifest 两种形态）
fn parse_release(v: &Value) -> (String, String, String, Value, String) {
    let latest = v["tag_name"].as_str()
        .or_else(|| v["version"].as_str())
        .or_else(|| v["name"].as_str())
        .unwrap_or("").to_string();
    let notes = v["body"].as_str()
        .or_else(|| v["notes"].as_str())
        .or_else(|| v["releaseNotes"].as_str())
        .unwrap_or("").to_string();
    let page = v["html_url"].as_str()
        .or_else(|| v["url"].as_str())
        .unwrap_or("").to_string();
    let published = v["published_at"].as_str()
        .or_else(|| v["publishedAt"].as_str())
        .unwrap_or("").to_string();
    let assets: Vec<Value> = v["assets"].as_array().map(|a| {
        a.iter().map(|x| json!({
            "name": x["name"].as_str().unwrap_or(""),
            "url": x["browser_download_url"].as_str().or_else(|| x["url"].as_str()).unwrap_or(""),
            "size": x["size"].as_u64().unwrap_or(0),
        })).collect()
    }).unwrap_or_default();
    (latest, notes, page, json!(assets), published)
}

async fn fetch_json(client: &reqwest::Client, url: &str, github_api: bool) -> Result<Value, String> {
    let mut req = client.get(url).timeout(Duration::from_secs(20))
        .header("User-Agent", format!("WebFixStudio/{}", VERSION));
    if github_api {
        req = req.header("Accept", "application/vnd.github+json");
    }
    let r = req.send().await.map_err(|e| format!("更新检测请求失败：{}", e))?;
    if r.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(Value::Null); // 尚无 Release：视为无更新
    }
    if !r.status().is_success() {
        return Err(format!("更新源返回 {}", r.status().as_u16()));
    }
    r.json::<Value>().await.map_err(|_| "更新源响应不是合法 JSON".to_string())
}

/// 检测更新：返回当前 / 最新版本、是否有更新、更新说明、下载资产列表
pub async fn check(client: &reqwest::Client) -> Result<Value, String> {
    let cfg = load_config();
    let current = VERSION;
    let (latest, notes, page, assets, published, source) = if !cfg.manifest_url.is_empty() {
        if !is_http(&cfg.manifest_url) {
            return Err("更新渠道地址必须以 http:// 或 https:// 开头".into());
        }
        let v = fetch_json(client, &cfg.manifest_url, false).await?;
        if v.is_null() {
            (current.to_string(), String::new(), String::new(), json!([]), String::new(), "manifest".to_string())
        } else {
            let (l, n, p, a, d) = parse_release(&v);
            (l, n, p, a, d, "manifest".to_string())
        }
    } else {
        let url = format!("https://api.github.com/repos/{}/releases/latest", REPO);
        let v = fetch_json(client, &url, true).await?;
        if v.is_null() {
            (current.to_string(), String::new(), String::new(), json!([]), String::new(), "github".to_string())
        } else {
            let (l, n, p, a, d) = parse_release(&v);
            (l, n, p, a, d, "github".to_string())
        }
    };
    let latest = if latest.is_empty() { current.to_string() } else { latest };
    Ok(json!({
        "current": current,
        "latest": latest,
        "hasUpdate": newer(&latest, current),
        "notes": notes,
        "url": page,
        "assets": assets,
        "publishedAt": published,
        "source": source,
        "repo": REPO,
        "autoCheck": cfg.auto_check,
        "manifestUrl": cfg.manifest_url,
    }))
}

/// 下载更新包到本地（config_dir/updates），返回路径与所在目录，供用户手动安装
pub async fn download(client: &reqwest::Client, url: &str, name: &str) -> Result<Value, String> {
    if !is_http(url) {
        return Err("下载地址必须以 http:// 或 https:// 开头".into());
    }
    let fname = safe_name(name);
    let dir = config_dir().join("updates");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建更新目录失败：{}", e))?;
    let r = client.get(url).timeout(Duration::from_secs(600))
        .header("User-Agent", format!("WebFixStudio/{}", VERSION))
        .send().await.map_err(|e| format!("下载请求失败：{}", e))?;
    if !r.status().is_success() {
        return Err(format!("下载源返回 {}", r.status().as_u16()));
    }
    if let Some(len) = r.content_length() {
        if len > MAX_DOWNLOAD_BYTES {
            return Err("更新包过大（超过 256MB），已拒绝下载".into());
        }
    }
    let bytes = r.bytes().await.map_err(|e| format!("下载失败：{}", e))?;
    if bytes.len() as u64 > MAX_DOWNLOAD_BYTES {
        return Err("更新包过大（超过 256MB），已拒绝下载".into());
    }
    let path = dir.join(&fname);
    std::fs::write(&path, &bytes).map_err(|e| format!("写入更新包失败：{}", e))?;
    Ok(json!({
        "ok": true,
        "name": fname,
        "path": path.to_string_lossy(),
        "dir": dir.to_string_lossy(),
        "size": bytes.len(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_compare() {
        assert!(newer("1.2.0", "1.1.0"));
        assert!(newer("v1.2.0", "1.1.9"));
        assert!(newer("2.0.0", "1.9.9"));
        assert!(!newer("1.1.0", "1.1.0"));
        assert!(!newer("1.0.0", "1.1.0"));
        assert!(!newer("1.2.0-beta", "1.2.0"));
        assert!(newer("1.10.0", "1.9.0"));
    }

    #[test]
    fn name_sanitization() {
        assert_eq!(safe_name("WebFixStudio-1.2.0-交付.zip"), "WebFixStudio-1.2.0-__.zip");
        assert_eq!(safe_name("../../evil.exe"), "evil.exe");
        assert_eq!(safe_name("a/b/c.zip"), "c.zip");
        assert_eq!(safe_name(""), "webfix-update.bin");
        assert_eq!(safe_name(".."), "webfix-update.bin");
    }

    #[test]
    fn parse_github_release_shape() {
        let v = json!({
            "tag_name": "v1.2.0", "body": "更新说明", "html_url": "https://github.com/x/y/releases/tag/v1.2.0",
            "published_at": "2026-11-14T00:00:00Z",
            "assets": [{ "name": "a.zip", "browser_download_url": "https://x/a.zip", "size": 123 }]
        });
        let (l, n, p, a, d) = parse_release(&v);
        assert_eq!(l, "v1.2.0");
        assert_eq!(n, "更新说明");
        assert!(p.contains("github.com"));
        assert_eq!(a[0]["url"], "https://x/a.zip");
        assert_eq!(a[0]["size"], 123);
        assert!(!d.is_empty());
    }

    #[test]
    fn parse_simple_manifest_shape() {
        let v = json!({ "version": "1.3.0", "notes": "n", "url": "https://x/page", "assets": [{ "name": "b.zip", "url": "https://x/b.zip", "size": 9 }] });
        let (l, n, _p, a, _d) = parse_release(&v);
        assert_eq!(l, "1.3.0");
        assert_eq!(n, "n");
        assert_eq!(a[0]["url"], "https://x/b.zip");
    }
}