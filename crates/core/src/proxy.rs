// 巡检代理：解除 X-Frame-Options / CSP 限制嵌入目标站，Cookie 按站隔离，注入抓取钩子
use crate::kb::enrich_findings;
use crate::scanner::{scan_headers, scan_html};
use crate::util::{b64url_decode, b64url_encode, origin_of};
use crate::AppState;
use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, Method, Request, Response, StatusCode};
use regex::Regex;
use reqwest::header::SET_COOKIE;
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use std::sync::OnceLock;
use std::time::Duration;
use url::Url;

const UA: &str = "Mozilla/5.0 (WebFixStudio-Inspect)";
const MAX_BODY: usize = 25 * 1024 * 1024;

macro_rules! re {
    ($name:ident, $pat:expr) => {
        fn $name() -> &'static Regex {
            static R: OnceLock<Regex> = OnceLock::new();
            R.get_or_init(|| Regex::new($pat).unwrap())
        }
    };
}

re!(re_proxy_path, r"^/proxy/([^/]+)(/.*)?$");
re!(re_attr, r#"(?i)(\s(?:href|src|action|poster)\s*=\s*)(["'])([^"']*)(["'])"#);
re!(re_srcset, r#"(?i)(\ssrcset\s*=\s*)(["'])([^"']*)(["'])"#);
re!(re_style_block, r#"(?is)<style\b[^>]*>.*?</style>"#);
re!(re_css_url, r#"url\(\s*(['"]?)([^'")]+)(['"]?)\s*\)"#);
re!(re_head_open, r#"(?i)<head[^>]*>"#);
re!(re_skip_scheme, r#"(?i)^(data:|javascript:|mailto:|tel:|about:|#)"#);

pub fn token_for(origin: &str) -> String {
    b64url_encode(origin)
}

pub fn origin_for(token: &str) -> Option<String> {
    let decoded = b64url_decode(token)?;
    let u = Url::parse(&decoded).ok()?;
    origin_of(&u)
}

pub fn cookie_prefix(token: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(token.as_bytes());
    let hex = format!("{:x}", hasher.finalize());
    format!("wf{}_", &hex[..8])
}

fn parse_cookies(header: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for part in header.split(';') {
        if let Some(eq) = part.find('=') {
            if eq > 0 {
                out.push((part[..eq].trim().to_string(), part[eq + 1..].trim().to_string()));
            }
        }
    }
    out
}

fn rewrite_set_cookies(raw: &[String], token: &str) -> Vec<String> {
    let prefix = cookie_prefix(token);
    raw.iter().map(|c| {
        let mut parts = c.split(';');
        let pair = parts.next().unwrap_or("");
        let eq = pair.find('=').unwrap_or(0);
        let name = pair[..eq].trim();
        let rest = &pair[eq..];
        let kept: Vec<String> = parts.map(|a| a.trim().to_string())
            .filter(|a| {
                if a.is_empty() { return false; }
                let la = a.to_lowercase();
                !(la.starts_with("domain=") || la.starts_with("path=") || la.starts_with("secure") || la.starts_with("samesite="))
            })
            .collect();
        let mut out = format!("{}{}{}; Path=/proxy/{}", prefix, name, rest, token);
        for k in kept {
            out.push_str("; ");
            out.push_str(&k);
        }
        out
    }).collect()
}

fn proxify_url(raw: &str, base: &Url) -> String {
    if raw.is_empty() || re_skip_scheme().is_match(raw) {
        return raw.to_string();
    }
    let Ok(u) = base.join(raw) else { return raw.to_string() };
    if u.scheme() != "http" && u.scheme() != "https" {
        return raw.to_string();
    }
    let Some(origin) = origin_of(&u) else { return raw.to_string() };
    let mut out = format!("/proxy/{}{}", token_for(&origin), u.path());
    if let Some(q) = u.query() {
        out.push('?');
        out.push_str(q);
    }
    if let Some(f) = u.fragment() {
        out.push('#');
        out.push_str(f);
    }
    out
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn rewrite_html(html: &str, page_url: &str, token: &str, sid: &str) -> String {
    let Ok(base) = Url::parse(page_url) else { return html.to_string() };

    // href / src / action / poster（闭合引号必须与开引号一致，等价 JS 反向引用 \2）
    let out = re_attr().replace_all(html, |c: &regex::Captures| {
        if &c[2] != &c[4] {
            return c[0].to_string();
        }
        format!("{}{}{}{}", &c[1], &c[2], proxify_url(&c[3], &base), &c[2])
    }).to_string();

    // srcset
    let out = re_srcset().replace_all(&out, |c: &regex::Captures| {
        if &c[2] != &c[4] {
            return c[0].to_string();
        }
        let rewritten = c[3].split(',').map(|part| {
            let mut seg: Vec<String> = part.trim().split_whitespace().map(String::from).collect();
            if let Some(first) = seg.first_mut() {
                *first = proxify_url(first, &base);
            }
            seg.join(" ")
        }).collect::<Vec<_>>().join(", ");
        format!("{}{}{}{}", &c[1], &c[2], rewritten, &c[2])
    }).to_string();

    // <style> 内 url(...)
    let out = re_style_block().replace_all(&out, |block: &regex::Captures| {
        re_css_url().replace_all(&block[0], |c: &regex::Captures| {
            if &c[1] != &c[3] {
                return c[0].to_string();
            }
            format!("url({}{}{})", &c[1], proxify_url(&c[2], &base), &c[1])
        }).to_string()
    }).to_string();

    if !out.contains("data-webfix-hook") {
        let hook = format!(
            "<script src=\"/capture-hook.js?token={}&target={}&sid={}\" data-webfix-hook></script>",
            token,
            percent_encoding::utf8_percent_encode(&origin_of(&base).unwrap_or_default(), percent_encoding::NON_ALPHANUMERIC),
            percent_encoding::utf8_percent_encode(sid, percent_encoding::NON_ALPHANUMERIC),
        );
        if let Some(m) = re_head_open().find(&out) {
            let mut s = out.clone();
            s.insert_str(m.end(), &hook);
            return s;
        }
        return format!("{}{}", hook, out);
    }
    out
}

fn drop_header(name: &str) -> bool {
    matches!(name,
        "x-frame-options" | "content-security-policy" | "content-security-policy-report-only"
        | "content-length" | "content-encoding" | "transfer-encoding" | "connection"
        | "keep-alive" | "strict-transport-security" | "alt-svc")
}

fn text_response(status: StatusCode, body: String) -> Response<Body> {
    Response::builder()
        .status(status)
        .header("Content-Type", "text/plain; charset=utf-8")
        .body(Body::from(body))
        .unwrap()
}

/// 巡检代理主入口（任意方法 /proxy/<token>/<path>）
pub async fn handle_proxy(state: &AppState, req: Request<Body>) -> Response<Body> {
    let path = req.uri().path().to_string();
    let search = req.uri().query().map(|q| format!("?{}", q)).unwrap_or_default();
    let Some(caps) = re_proxy_path().captures(&path) else {
        return text_response(StatusCode::NOT_FOUND, "not found".into());
    };
    let token = caps[1].to_string();
    let Some(origin) = origin_for(&token) else {
        return text_response(StatusCode::BAD_REQUEST, "无效的巡检令牌".into());
    };
    let rest = caps.get(2).map(|m| m.as_str()).unwrap_or("/").to_string();
    if rest.starts_with("/proxy") {
        return text_response(StatusCode::BAD_REQUEST, "禁止代理巡检地址自身（防循环）".into());
    }
    let target_url = format!("{}{}{}", origin, rest, search);
    let prefix = cookie_prefix(&token);

    let cookie_header = req.headers().get("cookie").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let jar = parse_cookies(&cookie_header);
    let sid = jar.iter().find(|(k, _)| k == &format!("{}sid", prefix)).map(|(_, v)| v.clone()).unwrap_or_default();
    let forward_cookies: String = jar.iter()
        .filter(|(k, _)| k.starts_with(&prefix) && k != &format!("{}sid", prefix))
        .map(|(k, v)| format!("{}={}", &k[prefix.len()..], v))
        .collect::<Vec<_>>()
        .join("; ");

    let method = req.method().clone();
    let req_headers = req.headers().clone();
    let body_bytes = if method == Method::POST || method == Method::PUT || method == Method::PATCH {
        axum::body::to_bytes(req.into_body(), 10 * 1024 * 1024).await.unwrap_or_default()
    } else {
        axum::body::Bytes::new()
    };

    // 透传业务请求头（apikey / Authorization 等），否则目标站接口会 401
    let skip = ["host", "connection", "cookie", "content-length", "accept-encoding",
        "origin", "referer", "user-agent", "accept", "accept-language"];
    let mut builder = state.manual.request(method.clone(), &target_url)
        .timeout(Duration::from_secs(20));
    for (k, v) in req_headers.iter() {
        let name = k.as_str();
        if skip.contains(&name) || name.starts_with("sec-") {
            continue;
        }
        if let Ok(val) = v.to_str() {
            builder = builder.header(name, val);
        }
    }
    let ua = req_headers.get("user-agent").and_then(|v| v.to_str().ok()).unwrap_or(UA);
    let accept = req_headers.get("accept").and_then(|v| v.to_str().ok()).unwrap_or("*/*");
    let accept_lang = req_headers.get("accept-language").and_then(|v| v.to_str().ok()).unwrap_or("zh-CN,zh;q=0.9");
    builder = builder.header("User-Agent", ua)
        .header("Accept", accept)
        .header("Accept-Language", accept_lang)
        .header("Referer", format!("{}/", origin));
    if !forward_cookies.is_empty() {
        builder = builder.header("Cookie", forward_cookies);
    }
    if !body_bytes.is_empty() {
        builder = builder.body(body_bytes.to_vec());
    }

    let upstream = match builder.send().await {
        Ok(r) => r,
        Err(e) => {
            let html = format!(
                "<h2>无法访问目标</h2><p>{}</p><p>{}</p><p>请确认目标站可访问；HTTPS 证书错误也会导致失败。</p>",
                html_escape(&target_url), html_escape(&e.to_string()));
            return Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .header("Content-Type", "text/html; charset=utf-8")
                .body(Body::from(html))
                .unwrap();
        }
    };

    let status = upstream.status();
    let up_headers = upstream.headers().clone();
    let raw_cookies: Vec<String> = up_headers.get_all(SET_COOKIE).iter()
        .filter_map(|v| v.to_str().ok().map(String::from)).collect();
    let set_cookies = rewrite_set_cookies(&raw_cookies, &token);

    // 重定向：Location 改写回代理路径
    if matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308) {
        let loc = up_headers.get("location").and_then(|v| v.to_str().ok()).unwrap_or("/");
        let new_loc = Url::parse(loc)
            .or_else(|_| Url::parse(&target_url).unwrap().join(loc))
            .ok()
            .and_then(|u| {
                let o = origin_of(&u)?;
                let mut s = format!("/proxy/{}{}", token_for(&o), u.path());
                if let Some(q) = u.query() { s.push('?'); s.push_str(q); }
                Some(s)
            })
            .unwrap_or_else(|| loc.to_string());
        let mut builder = Response::builder().status(status).header("Location", new_loc);
        for c in &set_cookies {
            builder = builder.header("Set-Cookie", c);
        }
        return builder.body(Body::empty()).unwrap();
    }

    let ct = up_headers.get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let buf = match upstream.bytes().await {
        Ok(b) => b,
        Err(e) => return text_response(StatusCode::BAD_GATEWAY, format!("读取响应失败：{}", e)),
    };
    if buf.len() > MAX_BODY {
        return text_response(StatusCode::BAD_GATEWAY, "响应过大".into());
    }

    let mut resp_headers = HeaderMap::new();
    for (k, v) in up_headers.iter() {
        let name = k.as_str();
        if drop_header(name) || name == "set-cookie" {
            continue;
        }
        if let Ok(val) = HeaderValue::from_bytes(v.as_bytes()) {
            resp_headers.append(k.clone(), val);
        }
    }
    for c in &set_cookies {
        if let Ok(val) = HeaderValue::from_str(c) {
            resp_headers.append(SET_COOKIE, val);
        }
    }
    resp_headers.insert("cache-control", HeaderValue::from_static("no-store"));

    if !sid.is_empty() && status.as_u16() >= 400 && ct.contains("text/html") {
        state.add_capture(&sid, json!({
            "type": "network", "url": target_url,
            "message": format!("页面返回 HTTP {}", status.as_u16()), "status": status.as_u16(),
        })).await;
    }

    if ct.contains("text/html") {
        let html = String::from_utf8_lossy(&buf).to_string();
        if !sid.is_empty() {
            let mut raw = scan_headers(&up_headers, &target_url, &raw_cookies);
            raw.extend(scan_html(&html, &target_url));
            if !raw.is_empty() {
                state.add_capture(&sid, json!({
                    "type": "scan", "url": target_url, "findings": enrich_findings(&raw),
                })).await;
            }
        }
        let rewritten = rewrite_html(&html, &target_url, &token, &sid);
        resp_headers.insert("content-type", HeaderValue::from_static("text/html; charset=utf-8"));
        let mut builder = Response::builder().status(status);
        for (k, v) in resp_headers.iter() {
            builder = builder.header(k.clone(), v.clone());
        }
        return builder.body(Body::from(rewritten)).unwrap();
    }

    let mut builder = Response::builder().status(status);
    for (k, v) in resp_headers.iter() {
        builder = builder.header(k.clone(), v.clone());
    }
    builder.body(Body::from(buf.to_vec())).unwrap()
}

/// 抓取事件中的代理地址 → 真实目标地址（与 JS realUrlOf 对齐）
pub fn real_url_of(ev: &Value) -> String {
    let page_url = ev["pageUrl"].as_str().unwrap_or("");
    let mut page_origin = String::new();
    let mut page_path = "/".to_string();
    let base_fallback = Url::parse("http://localhost").unwrap();
    if let Ok(pu) = Url::parse(page_url).or_else(|_| base_fallback.join(page_url)) {
        if let Some(pm) = re_proxy_path().captures(pu.path()) {
            page_origin = origin_for(&pm[1]).unwrap_or_default();
            let q = pu.query().map(|x| format!("?{}", x)).unwrap_or_default();
            page_path = format!("{}{}", pm.get(2).map(|m| m.as_str()).unwrap_or("/"), q);
        }
    }
    let page_real = if !page_origin.is_empty() {
        format!("{}{}", page_origin, page_path)
    } else {
        page_url.to_string()
    };

    let req_url = ev["url"].as_str().unwrap_or("");
    if !req_url.is_empty() {
        let base = if page_real.is_empty() { "http://localhost/".to_string() } else { page_real.clone() };
        if let Ok(u) = Url::parse(req_url).or_else(|_| Url::parse(&base).unwrap().join(req_url)) {
            if let Some(m) = re_proxy_path().captures(u.path()) {
                if let Some(o) = origin_for(&m[1]) {
                    let q = u.query().map(|x| format!("?{}", x)).unwrap_or_default();
                    return format!("{}{}{}", o, m.get(2).map(|x| x.as_str()).unwrap_or("/"), q);
                }
            }
            let is_tool_host = matches!(u.host_str(), Some("127.0.0.1") | Some("localhost"));
            let q = u.query().map(|x| format!("?{}", x)).unwrap_or_default();
            if is_tool_host && !page_origin.is_empty() {
                return format!("{}{}{}", page_origin, u.path(), q);
            }
            if !is_tool_host && (u.scheme() == "http" || u.scheme() == "https") {
                return u.to_string();
            }
            if !page_origin.is_empty() {
                if let Ok(joined) = Url::parse(&page_real).and_then(|b| b.join(req_url)) {
                    return joined.to_string();
                }
            }
        }
    }
    page_real
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_roundtrip() {
        let t = token_for("https://example.com");
        assert_eq!(origin_for(&t).as_deref(), Some("https://example.com"));
        assert!(origin_for("!!!invalid!!!").is_none());
    }

    #[test]
    fn cookie_isolation() {
        let t = token_for("https://a.com");
        let out = rewrite_set_cookies(&["session=abc; Domain=a.com; Path=/; Secure; HttpOnly; SameSite=Lax".into()], &t);
        assert!(out[0].starts_with("wf"));
        assert!(out[0].contains(&format!("Path=/proxy/{}", t)));
        assert!(!out[0].to_lowercase().contains("domain="));
        assert!(out[0].contains("HttpOnly"));
    }

    #[test]
    fn html_rewrite_and_hook() {
        let html = r#"<html><head><link href="/style.css"></head><body><a href="/x?a=1">x</a><img srcset="/a.png 1x, /b.png 2x"><style>.a{url('/bg.png')}</style></body></html>"#;
        let out = rewrite_html(html, "https://t.com/page", "TOKEN", "SID");
        let t = token_for("https://t.com");
        assert!(out.contains(&format!("/proxy/{}/style.css", t)));
        assert!(out.contains(&format!("/proxy/{}/x?a=1", t)));
        assert!(out.contains(&format!("/proxy/{}/a.png 1x", t)));
        assert!(out.contains(&format!("/proxy/{}/bg.png", t)));
        assert!(out.contains("data-webfix-hook"));
        assert!(out.contains("sid=SID"));
    }

    #[test]
    fn real_url_mapping() {
        let t = token_for("https://t.com");
        let ev = json!({
            "pageUrl": format!("http://127.0.0.1:8787/proxy/{}/admin", t),
            "url": "/api/user",
        });
        assert_eq!(real_url_of(&ev), "https://t.com/api/user");
    }
}