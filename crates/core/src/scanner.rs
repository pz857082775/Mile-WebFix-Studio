// 扫描引擎：HTML 启发式 + 响应头检查 + 敏感路径探测（与旧版 JS scanner.js 逐条对齐）
use crate::kb::{enrich_findings, summarize};
use crate::util::{now_iso, now_ms, origin_of, trunc};
use regex::Regex;
use reqwest::header::{HeaderMap, SET_COOKIE};
use reqwest::Client;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use url::Url;

pub const UA: &str = "WebFixStudio/1.0 (authorized-web-check)";

macro_rules! re {
    ($name:ident, $pat:expr) => {
        fn $name() -> &'static Regex {
            static R: OnceLock<Regex> = OnceLock::new();
            R.get_or_init(|| Regex::new($pat).unwrap())
        }
    };
}

re!(re_viewport, r#"(?i)<meta[^>]+name=["']?viewport"#);
re!(re_lang, r#"(?i)<html[^>]*\slang\s*="#);
re!(re_img, r#"(?i)<img\b[^>]*>"#);
re!(re_alt, r#"(?i)\balt\s*="#);
re!(re_id, r#"(?i)\sid\s*=\s*["']([^"']+)["']"#);
re!(re_input, r#"(?i)<input\b[^>]*>"#);
re!(re_input_skip, r#"(?i)type\s*=\s*["']?(hidden|submit|button|image|reset)"#);
re!(re_aria, r#"(?i)aria-label"#);
re!(re_handler, r#"(?i)\son[a-z]+\s*=\s*["'][^"']{0,80}"#);
re!(re_jsurl, r#"(?i)href\s*=\s*["']\s*javascript:"#);
re!(re_blank, r#"(?i)<a\b[^>]*target\s*=\s*["']_blank["'][^>]*>"#);
re!(re_noopener, r#"(?i)rel\s*=\s*["'][^"']*noopener"#);
re!(re_dangerous, r#"\beval\s*\(|document\.write\s*\(|\.innerHTML\s*="#);
re!(re_script, r#"(?i)<script\b[^>]*\ssrc\s*=\s*["'][^"']+["'][^>]*>"#);
re!(re_integrity, r#"(?i)\bintegrity\s*="#);
re!(re_mixed, r#"(?i)(?:src|href|action)\s*=\s*["']http://"#);
re!(re_form, r#"(?i)<form\b[^>]*>"#);
re!(re_form_http, r#"(?i)action\s*=\s*["']http://"#);
re!(re_pwd_input, r#"(?i)<input[^>]+type\s*=\s*["']?password"#);
re!(re_head, r#"(?is)<head\b.*?</head>"#);
re!(re_defer, r#"(?i)\b(defer|async)\b"#);
re!(re_module, r#"(?i)type\s*=\s*["']module"#);
re!(re_dim, r#"(?i)\b(width|height)\s*="#);
re!(re_autocomplete, r#"(?i)autocomplete\s*="#);
re!(re_post_form, r#"(?is)<form\b[^>]*>.*?</form>"#);
re!(re_method_post, r#"(?i)method\s*=\s*["']?post"#);
re!(re_csrf, r#"(?i)<input\b[^>]*type\s*=\s*["']?hidden[^>]*name\s*=\s*["']?(_csrf|csrf|token|authenticity_token)"#);
re!(re_secret, r#"(?i)(?:api[_-]?key|apikey|secret[_-]?key|access[_-]?token|auth[_-]?token|password|passwd|pwd)\s*[:=]\s*["'][A-Za-z0-9_\-./+=]{8,}["']|AKIA[0-9A-Z]{16}|-----BEGIN (?:RSA |EC )?PRIVATE KEY-----|eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{5,}"#);
re!(re_sqlerr, r#"(?i)SQL syntax|mysql_fetch|mysqli?_|ORA-\d{5}|PostgreSQL.*ERROR|SQLite3?::|sqlalchemy\.|JDBC|Unclosed quotation|quoted string not properly terminated|unrecognized token:|unterminated string literal"#);
re!(re_price_name, r#"(?i)name\s*=\s*["']?(price|amount|total|money|pay|fee|cost|discount)"#);
re!(re_idor, r#"(?i)(?:href|action)\s*=\s*["'][^"']*(?:[?&](?:id|uid|orderId|user_id)=\d+|/(?:order|user|file|invoice)s?/\d+)["']"#);
re!(re_pwd_url, r#"(?i)(?:href|action)\s*=\s*["'][^"']*[?&](?:pwd|password|passwd|token|secret)=[^"'&]+"#);
re!(re_csp_unsafe, r#"(?i)unsafe-inline|unsafe-eval"#);
re!(re_csp_wildcard, r#"(?i)(?:script-src|default-src)[^;]*\*"#);
re!(re_httponly, r#"(?i)httponly"#);
re!(re_secure, r#"(?i)secure"#);
re!(re_samesite, r#"(?i)samesite"#);
re!(re_anchor, r#"(?i)<a\b[^>]*\shref\s*=\s*["']([^"']+)["']"#);
re!(re_asset_ext, r#"(?i)\.(png|jpe?g|gif|svg|css|js|ico|pdf|zip|mp4|woff2?)$"#);
re!(re_sourcemap, r#"//[#@]\s*sourceMappingURL\s*=\s*(\S+)"#);
re!(re_script_src, r#"(?i)<script\b[^>]*\ssrc\s*=\s*["']([^"']+)["']"#);
re!(re_js_end, r#"(?i)\.js(\?|$)"#);
re!(re_js_replace, r#"(?i)\.js(\?.*)?$"#);
re!(re_trace, r#"(?i)TRACE"#);
re!(re_graphql, r#"(?i)__schema|queryType"#);
re!(re_hidden_input, r#"(?i)<input\b[^>]*type\s*=\s*["']?hidden[^>]*>"#);
re!(re_route_path, r#"(?:path|to|href|route|redirect|navigate|url)\s*[:=(]\s*["'](/[A-Za-z0-9_\-/:{}$.]{1,60})["']"#);
re!(re_route_str, r#"["'](/[a-z][A-Za-z0-9_\-/]{1,40})["']"#);
re!(re_img_src, r#"(?i)<img\b[^>]*\ssrc\s*=\s*["']([^"']+)["']"#);
re!(re_link_css, r#"(?i)<link\b[^>]*\shref\s*=\s*["']([^"']+)["'][^>]*>"#);
re!(re_redirect_js, r#"(?i)location\.href\s*=|window\.location\s*=|http-equiv\s*=\s*["']?refresh"#);
re!(re_sitemap_loc, r#"(?i)<loc>\s*([^<\s]+)\s*</loc>"#);

/// 反射 XSS 探测 canary（无害标记，仅检测是否被原样反射，不执行任何脚本）
const XSS_CANARY: &str = "wf7f3a\"><i>";
const REDIRECT_CANARY: &str = "https://example.com/wf-od";
const REDIRECT_PARAMS: &[&str] = &["url", "redirect", "redirect_url", "redirect_uri", "next", "return", "returnto", "return_url", "target", "goto", "continue", "dest", "destination", "jump", "rurl"];
const TRAVERSAL_PARAMS: &[&str] = &["file", "path", "page", "doc", "document", "template", "include", "load", "filename", "filepath", "dir", "folder"];

fn matches(r: &Regex, s: &str) -> Vec<String> {
    r.find_iter(s).map(|m| m.as_str().to_string()).collect()
}

pub struct SensitivePath {
    pub path: &'static str,
    pub label: &'static str,
    pub severity: &'static str,
}

pub const SENSITIVE_PATHS: &[SensitivePath] = &[
    SensitivePath { path: "/.env", label: "环境变量文件", severity: "critical" },
    SensitivePath { path: "/.git/config", label: "Git 配置泄露", severity: "critical" },
    SensitivePath { path: "/.git/HEAD", label: "Git 仓库泄露", severity: "critical" },
    SensitivePath { path: "/db.sql", label: "数据库备份", severity: "critical" },
    SensitivePath { path: "/backup.zip", label: "站点备份压缩包", severity: "high" },
    SensitivePath { path: "/web.config", label: "IIS 配置文件", severity: "high" },
    SensitivePath { path: "/.svn/entries", label: "SVN 元数据", severity: "high" },
    SensitivePath { path: "/actuator/env", label: "Spring Actuator 环境端点", severity: "high" },
    SensitivePath { path: "/phpinfo.php", label: "phpinfo 探针", severity: "medium" },
    SensitivePath { path: "/server-status", label: "Apache 状态页", severity: "medium" },
    SensitivePath { path: "/admin", label: "管理后台入口", severity: "medium" },
    SensitivePath { path: "/console", label: "调试控制台", severity: "medium" },
    SensitivePath { path: "/swagger-ui.html", label: "API 文档暴露", severity: "low" },
    SensitivePath { path: "/.DS_Store", label: "macOS 目录元数据", severity: "low" },
    SensitivePath { path: "/v2/api-docs", label: "Swagger v2 接口定义", severity: "medium" },
    SensitivePath { path: "/v3/api-docs", label: "OpenAPI v3 接口定义", severity: "medium" },
    SensitivePath { path: "/openapi.json", label: "OpenAPI 描述文件", severity: "medium" },
    SensitivePath { path: "/actuator", label: "Spring Actuator 端点列表", severity: "high" },
    SensitivePath { path: "/actuator/heapdump", label: "Actuator 堆转储（可提取密码）", severity: "critical" },
    SensitivePath { path: "/heapdump", label: "堆转储文件", severity: "critical" },
    SensitivePath { path: "/env", label: "环境变量端点", severity: "high" },
    SensitivePath { path: "/.gitlab-ci.yml", label: "GitLab CI 配置（含密钥）", severity: "high" },
    SensitivePath { path: "/docker-compose.yml", label: "Docker 编排配置（含口令）", severity: "high" },
    SensitivePath { path: "/wwwroot.zip", label: "站点源码压缩包", severity: "critical" },
    SensitivePath { path: "/backup.sql", label: "数据库备份 SQL", severity: "critical" },
    SensitivePath { path: "/database.sql", label: "数据库备份 SQL", severity: "critical" },
    SensitivePath { path: "/index.php.bak", label: "源码备份文件", severity: "high" },
    SensitivePath { path: "/config.bak", label: "配置备份文件", severity: "high" },
];

/// HTML 启发式检查（单页体检 / 爬行 / 巡检代理共用），输出 {ruleId, evidence}
pub fn scan_html(html: &str, url_href: &str) -> Vec<Value> {
    let mut raw: Vec<Value> = Vec::new();
    let mut push = |rule_id: &str, evidence: String| raw.push(json!({ "ruleId": rule_id, "evidence": evidence }));
    let url = Url::parse(url_href).unwrap_or_else(|_| Url::parse("http://localhost/").unwrap());
    if html.is_empty() {
        return raw;
    }

    if !re_viewport().is_match(html) {
        push("ui-missing-viewport", "未找到 <meta name=\"viewport\">，移动端可能布局错乱".into());
    }
    if !re_lang().is_match(html) {
        push("ui-missing-lang", "<html> 标签缺少 lang 属性".into());
    }

    let imgs = matches(re_img(), html);
    let no_alt: Vec<&String> = imgs.iter().filter(|t| !re_alt().is_match(t)).collect();
    if !no_alt.is_empty() {
        push("ui-img-no-alt", format!("{} 个 <img> 缺少 alt，例如：{}", no_alt.len(), trunc(no_alt[0], 100)));
    }

    let ids: Vec<String> = re_id().captures_iter(html).map(|c| c[1].to_string()).collect();
    let mut seen = std::collections::HashSet::new();
    let mut dups: Vec<String> = Vec::new();
    let mut dup_seen = std::collections::HashSet::new();
    for id in &ids {
        if !seen.insert(id.clone()) && dup_seen.insert(id.clone()) {
            dups.push(id.clone());
        }
    }
    if !dups.is_empty() {
        push("ui-duplicate-id", format!("重复的 id：{}", dups.join("、")));
    }

    let lower = html.to_lowercase();
    let label_re = regex::Regex::new(r"(?is)<label\b[^>]*>.*?</label\s*>").unwrap();
    let unlabeled: Vec<String> = re_input().find_iter(html).filter_map(|m| {
        let t = m.as_str();
        if re_input_skip().is_match(t) || re_aria().is_match(t) { return None; }
        if label_re.find_iter(html).any(|label| label.start() <= m.start() && label.end() >= m.end()) { return None; }
        if let Some(c) = re_id().captures(t) {
            let id = c[1].to_lowercase();
            if lower.contains(&format!("for=\"{}\"", id)) || lower.contains(&format!("for='{}'", id)) {
                return None;
            }
        }
        Some(t.to_string())
    }).collect();
    if !unlabeled.is_empty() {
        push("ui-input-no-label", format!("{} 个表单输入缺少 label/aria-label 关联，例如：{}", unlabeled.len(), trunc(&unlabeled[0], 100)));
    }

    let handlers = matches(re_handler(), html);
    if !handlers.is_empty() {
        push("sec-inline-handler", format!("{} 处内联事件处理器，例如：{}", handlers.len(), trunc(handlers[0].trim(), 100)));
    }
    if re_jsurl().is_match(html) {
        push("sec-js-url", "发现 javascript: 伪协议链接".into());
    }

    let blanks: Vec<String> = matches(re_blank(), html).into_iter().filter(|t| !re_noopener().is_match(t)).collect();
    if !blanks.is_empty() {
        push("sec-blank-noopener", format!("{} 个 target=\"_blank\" 链接缺少 rel=\"noopener\"", blanks.len()));
    }

    let dangerous = matches(re_dangerous(), html);
    if !dangerous.is_empty() {
        let mut uniq: Vec<String> = Vec::new();
        for d in &dangerous {
            if !uniq.contains(d) { uniq.push(d.clone()); }
        }
        push("sec-eval-dom", format!("发现 {} 处危险写法：{}", dangerous.len(), uniq.join("、")));
    }

    let scripts = matches(re_script(), html);
    let no_sri: Vec<&String> = scripts.iter().filter(|t| !re_integrity().is_match(t)).collect();
    if !no_sri.is_empty() {
        push("sec-missing-sri", format!("{} 个外部脚本缺少 integrity（SRI），例如：{}", no_sri.len(), trunc(no_sri[0], 100)));
    }

    if url.scheme() == "https" && re_mixed().is_match(html) {
        push("sec-mixed-content", "HTTPS 页面引用了 http:// 资源（混合内容）".into());
    }
    let forms = matches(re_form(), html);
    let http_forms: Vec<&String> = forms.iter().filter(|t| re_form_http().is_match(t)).collect();
    if !http_forms.is_empty() {
        push("sec-form-http", format!("表单通过明文 HTTP 提交：{}", trunc(http_forms[0], 120)));
    }
    if url.scheme() == "http" && re_pwd_input().is_match(html) {
        push("sec-password-http", "HTTP 页面存在密码输入框，凭据将明文传输".into());
    }

    let head = re_head().find(html).map(|m| m.as_str()).unwrap_or("");
    let blocking: Vec<String> = matches(re_script(), head).into_iter()
        .filter(|t| !re_defer().is_match(t) && !re_module().is_match(t)).collect();
    if !blocking.is_empty() {
        push("perf-render-blocking", format!("head 中 {} 个同步脚本阻塞渲染，例如：{}", blocking.len(), trunc(&blocking[0], 100)));
    }

    let no_dim: Vec<&String> = imgs.iter().filter(|t| !re_dim().is_match(t)).collect();
    if !no_dim.is_empty() {
        push("perf-cls-img", format!("{} 个 <img> 未声明宽高，加载时布局偏移（CLS），例如：{}", no_dim.len(), trunc(no_dim[0], 100)));
    }

    let pwd_no_ac: Vec<String> = matches(re_pwd_input(), html).into_iter()
        .filter(|t| !re_autocomplete().is_match(t)).collect();
    if !pwd_no_ac.is_empty() {
        push("sec-autocomplete-sensitive", format!("{} 个密码输入框未设置 autocomplete 策略", pwd_no_ac.len()));
    }

    let post_forms: Vec<String> = matches(re_post_form(), html).into_iter()
        .filter(|t| re_method_post().is_match(t))
        .filter(|t| !re_csrf().is_match(t)).collect();
    if !post_forms.is_empty() {
        push("sec-csrf-form", format!("{} 个 POST 表单缺少 CSRF token 隐藏字段", post_forms.len()));
    }

    let secrets = matches(re_secret(), html);
    if !secrets.is_empty() {
        push("sec-secret-js-inline", format!("{} 处疑似硬编码密钥/凭据，例如：{}", secrets.len(), trunc(&secrets[0], 80)));
    }

    if let Some(m) = re_sqlerr().find(html) {
        push("sec-sql-error-disclosure", format!("页面含数据库报错线索：{}", trunc(m.as_str(), 80)));
    }

    let price_hidden: Vec<String> = matches(re_hidden_input(), html).into_iter()
        .filter(|t| re_price_name().is_match(t)).collect();
    if !price_hidden.is_empty() {
        push("sec-price-hidden-field", format!("{} 个隐藏域承载价格/金额，例如：{}", price_hidden.len(), trunc(&price_hidden[0], 100)));
    }

    let idor_links = matches(re_idor(), html);
    if !idor_links.is_empty() {
        push("sec-idor-sequential", format!("{} 处顺序数字 ID 链接/接口，例如：{}", idor_links.len(), trunc(&idor_links[0], 100)));
    }

    if let Some(m) = re_pwd_url().find(html) {
        push("sec-password-in-url", format!("URL 中携带凭据参数：{}", trunc(m.as_str(), 100)));
    }
    raw
}

fn hget<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}

/// 响应头检查（单页体检 / 巡检代理共用）
pub fn scan_headers(headers: &HeaderMap, url_href: &str, cookies: &[String]) -> Vec<Value> {
    let mut raw: Vec<Value> = Vec::new();
    let mut push = |rule_id: &str, evidence: String| raw.push(json!({ "ruleId": rule_id, "evidence": evidence }));
    let Ok(url) = Url::parse(url_href) else { return raw };

    let csp = hget(headers, "content-security-policy");
    match csp {
        None => push("sec-missing-csp", "响应头中未包含 Content-Security-Policy".into()),
        Some(c) => {
            if re_csp_unsafe().is_match(c) || re_csp_wildcard().is_match(c) {
                push("sec-csp-unsafe", format!("CSP 过宽：{}", trunc(c, 160)));
            }
        }
    }
    if hget(headers, "x-frame-options").is_none() && !csp.unwrap_or("").contains("frame-ancestors") {
        push("sec-missing-xfo", "X-Frame-Options 与 CSP frame-ancestors 均未设置".into());
    }
    if hget(headers, "x-content-type-options").is_none() {
        push("sec-missing-xcto", "缺少 X-Content-Type-Options: nosniff".into());
    }
    if url.scheme() == "https" && hget(headers, "strict-transport-security").is_none() {
        push("sec-missing-hsts", "HTTPS 站点未设置 Strict-Transport-Security".into());
    }
    let disclosure: Vec<String> = ["server", "x-powered-by"].iter()
        .filter_map(|h| hget(headers, h).map(|v| format!("{}: {}", h, v))).collect();
    if !disclosure.is_empty() {
        push("sec-server-disclosure", disclosure.join("；"));
    }
    if hget(headers, "access-control-allow-origin") == Some("*") {
        push("sec-cors-wildcard", "Access-Control-Allow-Origin: *".into());
    }
    let mut cookie_issues: Vec<String> = Vec::new();
    for c in cookies {
        if !re_httponly().is_match(c) { cookie_issues.push("缺 HttpOnly".into()); }
        if url.scheme() == "https" && !re_secure().is_match(c) { cookie_issues.push("缺 Secure".into()); }
        if !re_samesite().is_match(c) { cookie_issues.push("缺 SameSite".into()); }
    }
    if !cookie_issues.is_empty() {
        let mut uniq: Vec<String> = Vec::new();
        for i in &cookie_issues {
            if !uniq.contains(i) { uniq.push(i.clone()); }
        }
        let first_name = cookies[0].split(';').next().unwrap_or("");
        push("sec-cookie-flags", format!("{} → {}", first_name, uniq.join("、")));
    }
    raw
}

pub fn extract_links(html: &str, base_url: &str) -> Vec<String> {
    let Ok(base) = Url::parse(base_url) else { return vec![] };
    let mut out: Vec<String> = Vec::new();
    for c in re_anchor().captures_iter(html) {
        let Ok(mut u) = base.join(&c[1]) else { continue };
        if origin_of(&u) != origin_of(&base) { continue; }
        if u.scheme() != "http" && u.scheme() != "https" { continue; }
        u.set_fragment(None);
        if re_asset_ext().is_match(u.path()) { continue; }
        let href = u.to_string();
        if !out.contains(&href) { out.push(href); }
    }
    out.truncate(200);
    out
}

/// SPA 路由发现：robots.txt + 同源 JS 包内的前端路由（react-router/vue-router 等），
/// 并顺带扫描 JS 包内硬编码密钥。返回 (补充路由, JS 发现)。
pub async fn discover_routes(html: &str, base_url: &str, follow: &Client, manual: &Client) -> (Vec<String>, Vec<Value>) {
    let Ok(base) = Url::parse(base_url) else { return (vec![], vec![]) };
    let Some(origin) = origin_of(&base) else { return (vec![], vec![]) };
    let mut links: Vec<String> = Vec::new();
    let mut findings: Vec<Value> = Vec::new();
    let push_path = |p: &str, links: &mut Vec<String>| {
        let p = p.trim();
        if p.is_empty() || p.contains("//") { return; }
        let Ok(mut u) = base.join(p) else { return };
        if origin_of(&u).as_deref() != Some(origin.as_str()) { return; }
        if u.scheme() != "http" && u.scheme() != "https" { return; }
        u.set_fragment(None);
        u.set_query(None);
        if re_asset_ext().is_match(u.path()) || u.path() == "/" { return; }
        let s = u.to_string();
        if !links.contains(&s) { links.push(s); }
    };

    // robots.txt 暴露的路径
    if let Ok(r) = manual.get(format!("{}/robots.txt", origin))
        .timeout(Duration::from_secs(5)).header("User-Agent", UA).send().await
    {
        if r.status().is_success() {
            let txt = r.text().await.unwrap_or_default();
            for line in txt.lines() {
                let l = line.trim();
                let low = l.to_lowercase();
                for pref in ["disallow:", "allow:", "sitemap:"] {
                    if low.starts_with(pref) {
                        let val = l[pref.len()..].trim();
                        if val.starts_with('/') {
                            push_path(val, &mut links);
                        } else if val.starts_with("http") {
                            if let Ok(u) = Url::parse(val) {
                                if origin_of(&u).as_deref() == Some(origin.as_str()) { push_path(u.path(), &mut links); }
                            }
                        }
                    }
                }
            }
        }
    }

    // 同源 JS 包：提取前端路由 + 硬编码密钥
    let scripts: Vec<String> = re_script_src().captures_iter(html).map(|c| c[1].to_string()).collect();
    let mut fetched = 0;
    for s in scripts {
        if fetched >= 6 { break; }
        if !re_js_end().is_match(&s) { continue; }
        let Ok(u) = base.join(&s) else { continue };
        if origin_of(&u).as_deref() != Some(origin.as_str()) { continue; }
        let Ok(r) = follow.get(u.as_str()).timeout(Duration::from_secs(8)).header("User-Agent", UA).send().await else { continue };
        if !r.status().is_success() { continue; }
        let js = r.text().await.unwrap_or_default();
        if js.len() > 3_000_000 { continue; }
        fetched += 1;
        for c in re_route_path().captures_iter(&js) { push_path(&c[1], &mut links); }
        for c in re_route_str().captures_iter(&js) { push_path(&c[1], &mut links); }
        if let Some(m) = re_secret().find(&js) {
            findings.push(json!({
                "ruleId": "sec-secret-js-inline",
                "evidence": format!("JS 包 {} 内疑似硬编码密钥：{}", u.path(), trunc(m.as_str(), 24)),
            }));
        }
    }
    links.truncate(120);
    (links, findings)
}

/// 智能路由发现：起始页 + 页内同源链接 + robots/JS 前端路由 + sitemap.xml，
/// 去重归一后返回可供「逐页深度探测」遍历的路由清单（含起始页本身）。
pub async fn discover_all_routes(follow: &Client, manual: &Client, url_href: &str) -> Vec<String> {
    let Ok(base) = Url::parse(url_href) else { return vec![] };
    let Some(origin) = origin_of(&base) else { return vec![] };
    let mut routes: Vec<String> = Vec::new();
    let push = |u: Url, routes: &mut Vec<String>| {
        if origin_of(&u).as_deref() != Some(origin.as_str()) { return; }
        if u.scheme() != "http" && u.scheme() != "https" { return; }
        let mut u = u;
        u.set_fragment(None);
        if re_asset_ext().is_match(u.path()) { return; }
        let s = u.to_string();
        if !routes.contains(&s) { routes.push(s); }
    };

    push(base.clone(), &mut routes);

    if let Ok(r) = follow.get(base.as_str()).timeout(Duration::from_secs(12)).header("User-Agent", UA).send().await {
        let final_url = r.url().to_string();
        let text = r.text().await.unwrap_or_default();
        for l in extract_links(&text, &final_url) {
            if let Ok(u) = Url::parse(&l) { push(u, &mut routes); }
        }
        let (extra, _) = discover_routes(&text, &final_url, follow, manual).await;
        for l in extra { if let Ok(u) = Url::parse(&l) { push(u, &mut routes); } }
    }

    if let Ok(r) = manual.get(format!("{}/sitemap.xml", origin)).timeout(Duration::from_secs(6)).header("User-Agent", UA).send().await {
        if r.status().is_success() {
            let xml = r.text().await.unwrap_or_default();
            for c in re_sitemap_loc().captures_iter(&xml) {
                if let Ok(u) = Url::parse(&c[1]) { push(u, &mut routes); }
            }
        }
    }

    routes.truncate(60);
    routes
}

fn set_cookies(headers: &HeaderMap) -> Vec<String> {
    headers.get_all(SET_COOKIE).iter().filter_map(|v| v.to_str().ok().map(String::from)).collect()
}

async fn probe_paths(client: &Client, origin: &str) -> Vec<&'static SensitivePath> {
    let queue: Arc<tokio::sync::Mutex<std::collections::VecDeque<&'static SensitivePath>>> =
        Arc::new(tokio::sync::Mutex::new(SENSITIVE_PATHS.iter().collect()));
    let hits: Arc<tokio::sync::Mutex<Vec<&'static SensitivePath>>> = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let mut workers = Vec::new();
    for _ in 0..4 {
        let q = queue.clone();
        let h = hits.clone();
        let client = client.clone();
        let origin = origin.to_string();
        workers.push(tokio::spawn(async move {
            loop {
                let item = { q.lock().await.pop_front() };
                let Some(item) = item else { break };
                let r = client.get(format!("{}{}", origin, item.path))
                    .timeout(Duration::from_secs(6))
                    .header("User-Agent", UA)
                    .send().await;
                if let Ok(r) = r {
                    if r.status() == 200 {
                        h.lock().await.push(item);
                    }
                }
            }
        }));
    }
    for w in workers { let _ = w.await; }
    let h = hits.lock().await;
    h.clone()
}

async fn probe_extra(client: &Client, origin: &str, html: &str) -> Vec<Value> {
    let mut raw: Vec<Value> = Vec::new();
    let base = format!("{}/", origin);

    // TRACE 方法（XST）
    if let Ok(r) = client.request(reqwest::Method::TRACE, &base)
        .timeout(Duration::from_secs(6)).header("User-Agent", UA).send().await
    {
        let status = r.status();
        let body = r.text().await.unwrap_or_default();
        if status == 200 && re_trace().is_match(&body) {
            raw.push(json!({ "ruleId": "sec-http-methods", "evidence": "TRACE 方法开启并回显请求（XST 风险）" }));
        }
    }

    // 生产 source map
    let mut candidates: Vec<String> = Vec::new();
    if let Some(c) = re_sourcemap().captures(html) {
        candidates.push(c[1].to_string());
    }
    let scripts: Vec<String> = re_script_src().captures_iter(html).map(|c| c[1].to_string()).collect();
    for s in scripts.iter().take(5) {
        if re_js_end().is_match(s) {
            candidates.push(re_js_replace().replace(s, ".js.map").to_string());
        }
    }
    for c in candidates.iter().take(3) {
        let Ok(mu) = Url::parse(&base).and_then(|b| b.join(c)) else { continue };
        if origin_of(&mu) != Some(origin.to_string()) { continue; }
        if let Ok(r) = client.get(mu.as_str()).timeout(Duration::from_secs(6)).header("User-Agent", UA).send().await {
            let status = r.status();
            let txt = if status == 200 { r.text().await.unwrap_or_default() } else { String::new() };
            if status == 200 && txt.contains("\"sources\"") {
                raw.push(json!({ "ruleId": "sec-source-map", "evidence": format!("GET {} → 200（生产 source map 可还原源码）", mu.path()) }));
                break;
            }
        }
    }

    // GraphQL 内省
    for gp in ["/graphql", "/api/graphql", "/v1/graphql"] {
        if let Ok(r) = client.post(format!("{}{}", origin, gp))
            .timeout(Duration::from_secs(6))
            .header("User-Agent", UA)
            .header("Content-Type", "application/json")
            .body(r#"{"query":"{__schema{queryType{name}}}"}"#)
            .send().await
        {
            let status = r.status();
            let txt = if status == 200 { r.text().await.unwrap_or_default() } else { String::new() };
            if status == 200 && re_graphql().is_match(&txt) {
                raw.push(json!({ "ruleId": "sec-graphql-introspection", "evidence": format!("POST {} 内省查询返回 schema（生产应关闭）", gp) }));
                break;
            }
        }
    }
    raw
}

pub struct ScanOpts {
    pub probe_paths: bool,
    /// 发现 SPA 前端路由（robots.txt + JS 包），用于全站爬行覆盖每个界面
    pub discover: bool,
    /// 主动无害漏洞探测（反射 XSS / SQLi 报错 / 开放重定向 / 路径遍历 / CORS 反射）
    pub active: bool,
    /// 链接 / 资源健康检查（死链、坏图、坏脚本样式）
    pub assets: bool,
}

impl Default for ScanOpts {
    fn default() -> Self {
        ScanOpts { probe_paths: false, discover: false, active: false, assets: false }
    }
}

fn with_param(url: &Url, pairs: &[(String, String)], name: &str, value: &str) -> Url {
    let mut u = url.clone();
    u.query_pairs_mut().clear().extend_pairs(
        pairs.iter().map(|(k, v)| (k.as_str(), if k == name { value } else { v.as_str() })),
    );
    u
}

fn get_form_probe_urls(html: &str, base_url: &str) -> Vec<String> {
    let Ok(base) = Url::parse(base_url) else { return Vec::new() };
    let action_re = regex::Regex::new(r#"(?i)\baction\s*=\s*["']([^"']*)["']"#).unwrap();
    let method_re = regex::Regex::new(r#"(?i)\bmethod\s*=\s*["']?([^\s"'>]+)"#).unwrap();
    let name_re = regex::Regex::new(r#"(?i)\bname\s*=\s*["']([^"']+)["']"#).unwrap();
    let mut urls = Vec::new();
    for form in re_post_form().find_iter(html).take(8) {
        let form = form.as_str();
        let Some(tag) = re_form().find(form).map(|m| m.as_str()) else { continue };
        if method_re.captures(tag).is_some_and(|m| !m[1].eq_ignore_ascii_case("get")) { continue; }
        let action = action_re.captures(tag).map(|m| m[1].to_string()).unwrap_or_default();
        let Ok(mut target) = base.join(&action) else { continue };
        if target.origin() != base.origin() || !matches!(target.scheme(), "http" | "https") { continue; }
        if target.path() != base.path() { continue; }
        let names: Vec<String> = re_input().find_iter(form)
            .filter(|m| !re_input_skip().is_match(m.as_str()) && !re_pwd_input().is_match(m.as_str()))
            .filter_map(|m| name_re.captures(m.as_str()).map(|c| c[1].to_string()))
            .filter(|name| ["q", "query", "search", "keyword", "term"].iter().any(|n| name.eq_ignore_ascii_case(n)))
            .take(4).collect();
        if names.is_empty() { continue; }
        target.query_pairs_mut().clear().extend_pairs(names.iter().map(|n| (n.as_str(), "")));
        let target = target.to_string();
        if !urls.contains(&target) { urls.push(target); }
        if urls.len() >= 4 { break; }
    }
    urls
}

/// 主动无害漏洞探测：仅发送带 canary 的 GET，不提交表单、不写数据、不执行脚本。
/// 反射 XSS / SQLi 报错 / 开放重定向 / 路径遍历 / CORS 反射。
pub async fn probe_active(manual: &Client, url_href: &str) -> Vec<Value> {
    let mut raw: Vec<Value> = Vec::new();
    let Ok(url) = Url::parse(url_href) else { return raw };
    let pairs: Vec<(String, String)> = url.query_pairs().map(|(k, v)| (k.into_owned(), v.into_owned())).collect();
    let names: Vec<String> = {
        let mut n: Vec<String> = Vec::new();
        for (k, _) in &pairs { if !n.contains(k) { n.push(k.clone()); } }
        n.truncate(4);
        n
    };

    // CORS 反射：发送任意 Origin，看是否被原样回显
    if let Ok(r) = manual.get(url.as_str()).timeout(Duration::from_secs(6))
        .header("User-Agent", UA).header("Origin", "https://wf-cors-test.example").send().await
    {
        let acao = hget(r.headers(), "access-control-allow-origin").unwrap_or("").to_string();
        let acac = hget(r.headers(), "access-control-allow-credentials").unwrap_or("").to_string();
        if acao == "https://wf-cors-test.example" || (acao == "*" && acac.eq_ignore_ascii_case("true")) {
            raw.push(json!({ "ruleId": "sec-cors-reflect", "evidence": format!("Access-Control-Allow-Origin 回显任意 Origin（{}），可被跨域读取", acao) }));
        }
    }

    let baseline_has_sql_error = if names.is_empty() {
        false
    } else if let Ok(r) = manual.get(url.as_str()).timeout(Duration::from_secs(6)).header("User-Agent", UA).send().await {
        re_sqlerr().is_match(&r.text().await.unwrap_or_default())
    } else {
        true
    };

    for name in &names {
        let orig = pairs.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone()).unwrap_or_default();
        // 反射 XSS：canary 原样（未转义）出现在响应中
        let xu = with_param(&url, &pairs, name, XSS_CANARY);
        if let Ok(r) = manual.get(xu.as_str()).timeout(Duration::from_secs(6)).header("User-Agent", UA).send().await {
            if r.status().is_success() {
                let body = r.text().await.unwrap_or_default();
                if body.contains(XSS_CANARY) {
                    raw.push(json!({ "ruleId": "sec-xss-reflected", "evidence": format!("参数 {} 的输入被未转义反射到响应（canary 原样回显）", name) }));
                }
            }
        }
        // SQLi 报错：追加单引号触发数据库报错泄露
        let su = with_param(&url, &pairs, name, &format!("{}'", orig));
        if let Ok(r) = manual.get(su.as_str()).timeout(Duration::from_secs(6)).header("User-Agent", UA).send().await {
            let body = r.text().await.unwrap_or_default();
            if !baseline_has_sql_error {
                if let Some(m) = re_sqlerr().find(&body) {
                    raw.push(json!({ "ruleId": "sec-sqli-error-based", "evidence": format!("{} 参数 {} 追加单引号后出现数据库报错，基线未见该错误：{}；请人工验证", url.path(), name, trunc(m.as_str(), 80)) }));
                }
            }
        }
        // 开放重定向
        if REDIRECT_PARAMS.iter().any(|p| p.eq_ignore_ascii_case(name)) {
            let ru = with_param(&url, &pairs, name, REDIRECT_CANARY);
            if let Ok(r) = manual.get(ru.as_str()).timeout(Duration::from_secs(6)).header("User-Agent", UA).send().await {
                let loc = hget(r.headers(), "location").unwrap_or("").to_string();
                let body = r.text().await.unwrap_or_default();
                if loc.starts_with(REDIRECT_CANARY) || (body.contains(REDIRECT_CANARY) && re_redirect_js().is_match(&body)) {
                    raw.push(json!({ "ruleId": "sec-open-redirect", "evidence": format!("参数 {} 可控制跳转目标到外部域（{}）", name, REDIRECT_CANARY) }));
                }
            }
        }
        // 路径遍历
        if TRAVERSAL_PARAMS.iter().any(|p| p.eq_ignore_ascii_case(name)) {
            let tu = with_param(&url, &pairs, name, "../../../../etc/passwd");
            if let Ok(r) = manual.get(tu.as_str()).timeout(Duration::from_secs(6)).header("User-Agent", UA).send().await {
                let body = r.text().await.unwrap_or_default();
                if body.contains("root:") || body.contains("[boot loader]") || body.contains("[fonts]") {
                    raw.push(json!({ "ruleId": "sec-path-traversal", "evidence": format!("参数 {} 可读取服务器本地文件（../../ 穿越命中）", name) }));
                }
            }
        }
    }
    raw
}

/// 链接 / 资源健康检查：同源页面死链 + 图片/脚本/样式坏链（真实功能性 BUG）
pub async fn probe_links(client: &Client, base_url: &str, html: &str, page_links: &[String]) -> Vec<Value> {
    let mut raw: Vec<Value> = Vec::new();
    let Ok(base) = Url::parse(base_url) else { return raw };
    let Some(origin) = origin_of(&base) else { return raw };

    let mut assets: Vec<String> = Vec::new();
    let push_asset = |s: &str, assets: &mut Vec<String>| {
        if s.is_empty() || s.starts_with("data:") { return; }
        let Ok(u) = base.join(s) else { return };
        if origin_of(&u).as_deref() != Some(origin.as_str()) { return; }
        let href = u.to_string();
        if !assets.contains(&href) { assets.push(href); }
    };
    for c in re_img_src().captures_iter(html) { push_asset(&c[1], &mut assets); }
    for c in re_script_src().captures_iter(html) { push_asset(&c[1], &mut assets); }
    for c in re_link_css().captures_iter(html) { push_asset(&c[1], &mut assets); }
    assets.truncate(12);
    for a in &assets {
        match client.get(a).timeout(Duration::from_secs(6)).header("User-Agent", UA).send().await {
            Ok(r) if r.status().as_u16() >= 400 => {
                raw.push(json!({ "ruleId": "func-resource-error", "evidence": format!("资源 HTTP {}：{}", r.status().as_u16(), a) }));
            }
            Err(_) => raw.push(json!({ "ruleId": "func-resource-error", "evidence": format!("资源无法加载：{}", a) })),
            _ => {}
        }
    }

    let mut checked = 0;
    for l in page_links {
        if checked >= 8 { break; }
        checked += 1;
        if let Ok(r) = client.get(l).timeout(Duration::from_secs(6)).header("User-Agent", UA).send().await {
            if r.status().as_u16() >= 400 {
                raw.push(json!({ "ruleId": "func-http-error", "evidence": format!("死链 HTTP {}：{}", r.status().as_u16(), l) }));
            }
        }
    }
    raw
}

/// 单页体检（与 JS scanTarget 输出结构一致）
pub async fn scan_target(follow: &Client, manual: &Client, raw_url: &str, opts: ScanOpts) -> Result<Value, String> {
    let started = now_ms();
    let url = Url::parse(raw_url).map_err(|_| "URL 格式不正确".to_string())?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err("仅支持 http/https 地址".into());
    }

    let res = follow.get(url.as_str())
        .timeout(Duration::from_secs(15))
        .header("User-Agent", UA)
        .header("Accept", "text/html,application/xhtml+xml,*/*")
        .send().await
        .map_err(|e| {
            if e.is_timeout() { "目标响应超时（15 秒）".to_string() }
            else { format!("无法访问目标：{}", e) }
        })?;

    let status = res.status();
    let final_url = res.url().to_string();
    let headers = res.headers().clone();
    let ct = hget(&headers, "content-type").unwrap_or("").to_string();
    let html = if ct.contains("text/html") {
        trunc(&res.text().await.unwrap_or_default(), 2_000_000)
    } else {
        String::new()
    };

    let mut raw: Vec<Value> = Vec::new();
    if status.as_u16() >= 400 {
        let reason = status.canonical_reason().unwrap_or("");
        raw.push(json!({ "ruleId": "func-http-error", "evidence": format!("HTTP {} {}", status.as_u16(), reason) }));
    }
    if url.scheme() == "http" {
        raw.push(json!({ "ruleId": "sec-no-https", "evidence": format!("整站使用明文 HTTP 传输：{}", url.as_str()) }));
    }

    let cookies = set_cookies(&headers);
    raw.extend(scan_headers(&headers, url.as_str(), &cookies));
    raw.extend(scan_html(&html, url.as_str()));

    let page_links = extract_links(&html, &final_url);
    let mut extra_links: Vec<String> = Vec::new();
    if opts.discover {
        let (el, jf) = discover_routes(&html, &final_url, follow, manual).await;
        extra_links = el;
        raw.extend(jf);
    }

    if opts.probe_paths {
        let origin = origin_of(&url).unwrap_or_default();
        let hits = probe_paths(manual, &origin).await;
        for h in hits {
            raw.push(json!({ "ruleId": "sec-sensitive-path", "evidence": format!("GET {} → 200（{}）", h.path, h.label) }));
        }
        let extra = tokio::time::timeout(Duration::from_secs(6), probe_extra(manual, &origin, &html))
            .await.unwrap_or_default();
        raw.extend(extra);
    }

    if opts.active {
        let active = tokio::time::timeout(Duration::from_secs(30), async {
            let mut findings = probe_active(manual, url.as_str()).await;
            for form_url in get_form_probe_urls(&html, &final_url) {
                if form_url != url.as_str() {
                    findings.extend(probe_active(manual, &form_url).await);
                }
            }
            findings
        }).await.unwrap_or_default();
        raw.extend(active);
    }
    if opts.assets {
        let health = tokio::time::timeout(Duration::from_secs(20), probe_links(follow, &final_url, &html, &page_links))
            .await.unwrap_or_default();
        raw.extend(health);
    }

    // 按 ruleId 合并证据
    let mut merged: Vec<Value> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for f in raw {
        let rid = f["ruleId"].as_str().unwrap_or("").to_string();
        if let Some(&i) = index.get(&rid) {
            let old = merged[i]["evidence"].as_str().unwrap_or("").to_string();
            let add = f["evidence"].as_str().unwrap_or("");
            if !old.lines().any(|line| line == add) {
                merged[i]["evidence"] = json!(format!("{}\n{}", old, add));
            }
        } else {
            index.insert(rid, merged.len());
            merged.push(f);
        }
    }

    let findings = enrich_findings(&merged);
    let mut links = page_links;
    for l in extra_links { if !links.contains(&l) { links.push(l); } }
    let mut hmap = Map::new();
    for (k, v) in headers.iter() {
        let val = v.to_str().unwrap_or("");
        let key = k.as_str().to_string();
        match hmap.get(&key) {
            Some(old) => {
                let joined = format!("{}, {}", old.as_str().unwrap_or(""), val);
                hmap.insert(key, json!(joined));
            }
            None => { hmap.insert(key, json!(val)); }
        }
    }

    Ok(json!({
        "target": url.as_str(),
        "finalUrl": final_url,
        "status": status.as_u16(),
        "scannedAt": now_iso(),
        "durationMs": now_ms() - started,
        "findings": findings,
        "links": links,
        "headers": Value::Object(hmap),
        "html": trunc(&html, 30_000),
    }))
}

pub fn with_summary(mut result: Value) -> Value {
    let findings = result["findings"].as_array().cloned().unwrap_or_default();
    result["summary"] = summarize(&findings);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEMO: &str = r#"<!doctype html><html><head><meta charset="utf-8"><script src="http://cdn.example.com/a.js"></script></head><body><h1 id="main">x</h1><p id="main">y</p><img src="/a.png"><form action="http://x/" method="GET"><input type="password" name="pwd"></form><button onclick="alert(1)">b</button><a href="javascript:void(0)">j</a><a href="https://e.com" target="_blank">n</a><script>document.getElementById('out').innerHTML = location.hash;</script></body></html>"#;

    #[test]
    fn html_rules_fire() {
        let raw = scan_html(DEMO, "http://localhost/demo");
        let ids: Vec<&str> = raw.iter().map(|f| f["ruleId"].as_str().unwrap()).collect();
        for expect in ["ui-missing-viewport", "ui-missing-lang", "ui-img-no-alt", "ui-duplicate-id",
            "ui-input-no-label", "sec-inline-handler", "sec-js-url", "sec-blank-noopener",
            "sec-eval-dom", "sec-missing-sri", "sec-form-http", "sec-password-http",
            "perf-render-blocking", "perf-cls-img", "sec-autocomplete-sensitive"] {
            assert!(ids.contains(&expect), "missing {}", expect);
        }
    }

    #[test]
    fn nested_label_associates_input() {
        let html = r#"<label>搜索<input name="q"></label><input name="unlabeled">"#;
        let issues = scan_html(html, "http://localhost/");
        let issue = issues.iter().find(|f| f["ruleId"] == "ui-input-no-label").unwrap();
        assert!(issue["evidence"].as_str().unwrap().contains("1 个"));
        assert!(issue["evidence"].as_str().unwrap().contains("unlabeled"));
    }

    #[test]
    fn get_form_discovery_stays_same_origin_and_skips_post() {
        let html = r#"<form><input name="q"></form>
            <form method="post"><input name="password" type="password"></form>
            <form action="https://other.example/search"><input name="q"></form>
            <form action="/search" method="get"><input name="term"></form>"#;
        assert_eq!(get_form_probe_urls(html, "http://localhost/products"), vec![
            "http://localhost/products?q="
        ]);
    }

    #[test]
    fn sqlite_error_signatures_match() {
        assert!(re_sqlerr().is_match("数据库异常：unrecognized token: &quot;&#39;&quot;"));
        assert!(!re_sqlerr().is_match("正常搜索结果"));
    }

    #[tokio::test]
    async fn sqli_probe_compares_with_baseline() {
        use axum::{extract::Query, routing::get, Router};
        let app = Router::new()
            .route("/vulnerable", get(|Query(q): Query<HashMap<String, String>>| async move {
                if q.get("q").is_some_and(|v| v.contains('\'')) { "unrecognized token:" } else { "ok" }
            }))
            .route("/baseline_error", get(|| async { "unrecognized token:" }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = Client::builder().redirect(reqwest::redirect::Policy::none()).build().unwrap();
        let detected = probe_active(&client, &format!("http://{addr}/vulnerable?q=hello")).await;
        assert!(detected.iter().any(|f| f["ruleId"] == "sec-sqli-error-based"));
        let existing = probe_active(&client, &format!("http://{addr}/baseline_error?q=hello")).await;
        assert!(!existing.iter().any(|f| f["ruleId"] == "sec-sqli-error-based"));
        server.abort();
    }

    #[test]
    fn secret_detection() {
        let html = r#"<script>const apiKey = "abcd1234efgh5678";</script>"#;
        let raw = scan_html(html, "https://x.com/");
        assert!(raw.iter().any(|f| f["ruleId"] == "sec-secret-js-inline"));
    }

    #[test]
    fn links_same_origin_only() {
        let html = r#"<a href="/a">a</a><a href="https://other.com/x">o</a><a href="/img.png">i</a><a href="/a#frag">d</a>"#;
        let links = extract_links(html, "https://x.com/page");
        assert_eq!(links, vec!["https://x.com/a"]);
    }
}