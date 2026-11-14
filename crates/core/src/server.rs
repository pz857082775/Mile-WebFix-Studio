// HTTP 服务：与 JS server.js 路由完全对齐 + 静态资源（rust-embed：debug 读盘 / release 内嵌）
use crate::extensions::{self, read_audit, discover_ext, invoke_ext, register_ext, remove_ext, update_ext};
use crate::orchestrator;
use crate::proxy;
use crate::scanner::{self, ScanOpts};
use crate::util::{err_json, is_local_url, now_ms, target_auth_error, trunc};
use crate::{crawler, fixer, kb, AppState};
use axum::body::{Body, Bytes};
use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use axum::{Json, Router};
use rust_embed::RustEmbed;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use url::Url;

#[derive(RustEmbed)]
#[folder = "../../web/dist/"]
struct Assets;

const NAV: &str = "<nav><a href=\"/demo\">首页</a> | <a href=\"/demo/about\">关于</a> | <a href=\"/demo/contact\">联系</a> | <a href=\"/demo/admin\">后台（需登录）</a></nav>";

fn demo_home() -> String {
    format!(r#"<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <title>演示靶场 · 故意含漏洞的页面</title>
  <script src="http://cdn.example.com/analytics.js"></script>
</head>
<body>
  {NAV}
  <h1 id="main">演示靶场</h1>
  <p id="main">本页面故意包含典型问题，用于演示扫描与修复流程。</p>
  <img src="/logo.png">
  <img src="/banner.png">
  <form action="http://127.0.0.1:8787/demo" method="GET">
    <input type="text" name="q" placeholder="搜索">
    <input type="password" name="pwd" placeholder="密码">
    <button type="submit">登录</button>
  </form>
  <button onclick="alert('inline handler')">内联事件</button>
  <a href="javascript:void(0)">javascript 链接</a>
  <a href="https://example.com" target="_blank">新窗口（无 noopener）</a>
  <div id="out"></div>
  <script>
    document.getElementById('out').innerHTML = location.hash;
  </script>
</body>
</html>"#)
}

fn demo_about() -> String {
    format!(r#"<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>关于 · 演示靶场</title>
</head>
<body>
  {NAV}
  <h1 id="t">关于我们</h1>
  <p id="t">本页演示运行时错误抓取：控制台报错、接口 404、资源加载失败、未捕获异常。</p>
  <img src="/missing-image.png">
  <script>
    console.error('演示：用户接口超时', {{ api: '/api/user', code: 504 }});
    fetch('/api/definitely-missing').catch(function () {{}});
    setTimeout(function () {{ window.__demoCrash.noSuchMethod(); }}, 800);
  </script>
</body>
</html>"#)
}

fn demo_contact() -> String {
    format!(r#"<!doctype html>
<html>
<head><meta charset="utf-8"><title>联系 · 演示靶场</title></head>
<body>
  {NAV}
  <h1>联系我们</h1>
  <form action="/demo/contact" method="GET">
    <input type="text" name="name" placeholder="姓名">
    <input type="email" name="email" placeholder="邮箱">
    <button onclick="alert('submitted')">提交</button>
  </form>
</body>
</html>"#)
}

fn demo_admin(authed: bool) -> String {
    if !authed {
        return format!(r#"<!doctype html>
<html lang="zh-CN">
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>后台登录 · 演示靶场</title></head>
<body>
  {NAV}
  <h1>管理后台 · 登录</h1>
  <p>演示「登录墙」：直接扫描只能看到登录页；用人工巡检登录后即可抓取后台页面的问题。</p>
  <form method="POST" action="/demo/login">
    <input type="text" name="user" placeholder="账号（任意）">
    <input type="password" name="pwd" placeholder="密码（任意）">
    <button type="submit">登录</button>
  </form>
</body>
</html>"#);
    }
    format!(r#"<!doctype html>
<html lang="zh-CN">
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>后台面板 · 演示靶场</title></head>
<body>
  {NAV}
  <h1>后台面板（已登录）</h1>
  <div id="report"></div>
  <script>
    console.error('演示：后台报表接口 500', {{ api: '/api/admin/report' }});
    document.getElementById('report').innerHTML = location.hash;
  </script>
</body>
</html>"#)
}

// ---- 响应工具 ----
fn json_ok(v: Value) -> Response {
    Json(v).into_response()
}

fn json_err(code: StatusCode, msg: impl Into<String>) -> Response {
    (code, Json(err_json(msg))).into_response()
}

fn text_ok(text: String, mime: &str) -> Response {
    ([(header::CONTENT_TYPE, mime.to_string())], text).into_response()
}

fn parse_body(bytes: &Bytes) -> Result<Value, Response> {
    if bytes.is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_slice(bytes).map_err(|e| json_err(StatusCode::BAD_REQUEST, format!("JSON 解析失败：{}", e)))
}

fn demo_response(html: String, extra_cookie: Option<&str>) -> Response {
    let mut resp = (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::HeaderName::from_static("server"), "WebFix-Demo/1.0"),
            (header::HeaderName::from_static("x-powered-by"), "DemoPHP/5.6"),
            (header::HeaderName::from_static("access-control-allow-origin"), "*"),
        ],
        html,
    ).into_response();
    if let Some(c) = extra_cookie {
        resp.headers_mut().insert(header::SET_COOKIE, c.parse().unwrap());
    }
    resp
}

fn auth_gate(url: Option<&str>, authorized: bool) -> Result<Url, Response> {
    let raw = url.unwrap_or("");
    if raw.is_empty() {
        return Err(json_err(StatusCode::BAD_REQUEST, "缺少 url 参数"));
    }
    let u = Url::parse(raw).map_err(|_| json_err(StatusCode::BAD_REQUEST, "URL 格式不正确"))?;
    if !is_local_url(&u) && !authorized {
        return Err(json_err(StatusCode::BAD_REQUEST, "请先确认已获得该网站的测试授权"));
    }
    Ok(u)
}

// ---- 路由 ----
pub fn router(state: Arc<AppState>) -> Router {
    let api = Router::new()
        .route("/api/health", get(|| async { json_ok(json!({ "ok": true })) }))
        .route("/api/scan", post(api_scan))
        .route("/api/crawl", post(api_crawl))
        .route("/api/crawl-status", get(api_crawl_status))
        .route("/api/inspect/start", post(api_inspect_start))
        .route("/api/capture", post(api_capture))
        .route("/api/captures", get(api_captures))
        .route("/api/inspect/clear", post(api_inspect_clear))
        .route("/api/scan-html", post(api_scan_html))
        .route("/api/fix", post(api_fix))
        .route("/api/prompt", post(api_prompt))
        .route("/api/patch", post(api_patch))
        .route("/api/report", post(api_report))
        .route("/api/rules", get(|| async { json_ok(json!({ "rules": kb::rules_json() })) }))
        .route("/api/ai/config", get(ai_config_get).post(ai_config_post))
        .route("/api/ai/test", post(ai_test))
        .route("/api/ai/models", post(ai_models))
        .route("/api/ai/analyze", post(ai_analyze))
        .route("/api/ext/meta", get(ext_meta))
        .route("/api/ext/list", get(|| async { json_ok(json!({ "extensions": extensions::load_store()["extensions"] })) }))
        .route("/api/ext/register", post(ext_register))
        .route("/api/ext/update", post(ext_update))
        .route("/api/ext/remove", post(ext_remove))
        .route("/api/ext/discover", post(ext_discover))
        .route("/api/ext/invoke", post(ext_invoke))
        .route("/api/audit", get(api_audit))
        .route("/api/orch/pipelines", get(|| async { json_ok(json!({ "pipelines": orchestrator::list_pipelines() })) }))
        .route("/api/orch/pipeline", post(orch_pipeline_save))
        .route("/api/orch/pipeline/remove", post(orch_pipeline_remove))
        .route("/api/orch/run", post(orch_run))
        .route("/api/orch/run-status", get(orch_run_status))
        .layer(axum::extract::DefaultBodyLimit::max(4 * 1024 * 1024));

    Router::new()
        .route("/demo", get(|| async { demo_response(demo_home(), Some("session=demo123; Path=/")) }))
        .route("/demo/about", get(|| async { demo_response(demo_about(), None) }))
        .route("/demo/contact", get(|| async { demo_response(demo_contact(), None) }))
        .route("/demo/admin", get(demo_admin_handler))
        .route("/demo/login", post(|| async {
            (
                StatusCode::FOUND,
                [(header::SET_COOKIE, "wf_demo_auth=1; Path=/"), (header::LOCATION, "/demo/admin")],
            ).into_response()
        }))
        .route("/sitemap.xml", get(sitemap))
        .route("/proxy/{*rest}", any(proxy_handler))
        .merge(api)
        .fallback(static_handler)
        .with_state(state)
}

// ---- 演示靶场 ----
async fn demo_admin_handler(headers: HeaderMap) -> Response {
    let cookie = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()).unwrap_or("");
    demo_response(demo_admin(cookie.contains("wf_demo_auth=1")), None)
}

async fn sitemap(headers: HeaderMap) -> Response {
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok()).unwrap_or("127.0.0.1:8787");
    let base = format!("http://{}", host);
    let urls: String = ["/demo", "/demo/about", "/demo/contact", "/demo/admin"]
        .iter().map(|p| format!("<url><loc>{}{}</loc></url>", base, p)).collect();
    text_ok(
        format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">{}</urlset>", urls),
        "application/xml; charset=utf-8",
    )
}

// ---- 巡检代理 ----
async fn proxy_handler(State(state): State<Arc<AppState>>, req: axum::extract::Request) -> Response {
    proxy::handle_proxy(&state, req).await
}

// ---- 单页体检 / 爬行 ----
async fn api_scan(State(state): State<Arc<AppState>>, body: Bytes) -> Response {
    let v = match parse_body(&body) { Ok(v) => v, Err(r) => return r };
    let url = match auth_gate(v["url"].as_str(), v["authorized"].as_bool().unwrap_or(false)) {
        Ok(u) => u, Err(r) => return r,
    };
    let opts = ScanOpts { probe_paths: v["probePaths"].as_bool().unwrap_or(false) };
    match scanner::scan_target(&state.follow, &state.manual, url.as_str(), opts).await {
        Ok(r) => json_ok(scanner::with_summary(r)),
        Err(e) => json_err(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

async fn api_crawl(State(state): State<Arc<AppState>>, body: Bytes) -> Response {
    let v = match parse_body(&body) { Ok(v) => v, Err(r) => return r };
    let url = match auth_gate(v["url"].as_str(), v["authorized"].as_bool().unwrap_or(false)) {
        Ok(u) => u, Err(r) => return r,
    };
    let job = crawler::start_crawl(
        state.follow.clone(), state.manual.clone(), url.as_str(),
        v["maxPages"].as_u64(), v["maxDepth"].as_u64(),
    );
    let id = job.lock().await.id.clone();
    state.crawls.lock().await.insert(id.clone(), job);
    json_ok(json!({ "id": id }))
}

async fn api_crawl_status(State(state): State<Arc<AppState>>, Query(q): Query<HashMap<String, String>>) -> Response {
    let id = q.get("id").cloned().unwrap_or_default();
    let job = state.crawls.lock().await.get(&id).cloned();
    match job {
        Some(j) => {
            let guard = j.lock().await;
            json_ok(crawler::job_json(&guard))
        }
        None => json_err(StatusCode::NOT_FOUND, "任务不存在"),
    }
}

// ---- 人工巡检 ----
async fn api_inspect_start(State(state): State<Arc<AppState>>, body: Bytes) -> Response {
    let v = match parse_body(&body) { Ok(v) => v, Err(r) => return r };
    let url = match auth_gate(v["url"].as_str(), v["authorized"].as_bool().unwrap_or(false)) {
        Ok(u) => u, Err(r) => return r,
    };
    let sid = uuid::Uuid::new_v4().to_string();
    let origin = crate::util::origin_of(&url).unwrap_or_default();
    let token = proxy::token_for(&origin);
    let proxy_url = format!("/proxy/{}{}{}", token, url.path(), url.query().map(|s| format!("?{}", s)).unwrap_or_default());
    state.sessions.lock().await.insert(sid.clone(), crate::Session {
        events: vec![], created_at: now_ms(), target: Some(url.to_string()),
    });
    json_ok(json!({
        "sid": sid,
        "token": token,
        "proxyUrl": proxy_url,
        "cookieName": format!("{}sid", proxy::cookie_prefix(&token)),
        "cookiePath": format!("/proxy/{}", token),
    }))
}

async fn api_capture(State(state): State<Arc<AppState>>, body: Bytes) -> Response {
    let v = match parse_body(&body) { Ok(v) => v, Err(r) => return r };
    let sid = v["sid"].as_str().unwrap_or("").to_string();
    if !sid.is_empty() {
        state.add_capture(&sid, v).await;
    }
    json_ok(json!({ "ok": true }))
}

async fn api_captures(State(state): State<Arc<AppState>>, Query(q): Query<HashMap<String, String>>) -> Response {
    let sid = q.get("sid").cloned().unwrap_or_default();
    let sessions = state.sessions.lock().await;
    let Some(s) = sessions.get(&sid) else {
        return json_err(StatusCode::NOT_FOUND, "会话不存在");
    };
    let events: Vec<Value> = s.events.iter().map(|ev| {
        let mut e = ev.clone();
        e["realUrl"] = json!(proxy::real_url_of(ev));
        e
    }).collect();
    json_ok(json!({ "events": events, "target": s.target }))
}

async fn api_inspect_clear(State(state): State<Arc<AppState>>, body: Bytes) -> Response {
    let v = match parse_body(&body) { Ok(v) => v, Err(r) => return r };
    if let Some(sid) = v["sid"].as_str() {
        if let Some(s) = state.sessions.lock().await.get_mut(sid) {
            s.events.clear();
        }
    }
    json_ok(json!({ "ok": true }))
}

async fn api_scan_html(body: Bytes) -> Response {
    let v = match parse_body(&body) { Ok(v) => v, Err(r) => return r };
    let html = trunc(v["html"].as_str().unwrap_or(""), 2_000_000);
    let url = v["url"].as_str().filter(|u| !u.is_empty()).unwrap_or("http://localhost/");
    let findings = kb::enrich_findings(&scanner::scan_html(&html, url));
    json_ok(json!({ "findings": findings }))
}

// ---- 修复四件套 ----
fn stack_of(v: &Value) -> Value {
    json!({
        "frontend": v["stack"]["frontend"].as_str().filter(|s| !s.is_empty()).unwrap_or("原生 HTML/JS"),
        "backend": v["stack"]["backend"].as_str().filter(|s| !s.is_empty()).unwrap_or("Node.js (Express)"),
    })
}

fn finding_of(v: &Value) -> Value {
    if v["finding"].is_object() { v["finding"].clone() } else { json!({}) }
}

async fn api_fix(body: Bytes) -> Response {
    let v = match parse_body(&body) { Ok(v) => v, Err(r) => return r };
    json_ok(fixer::build_deliverables(&finding_of(&v), &stack_of(&v)))
}

async fn api_prompt(body: Bytes) -> Response {
    let v = match parse_body(&body) { Ok(v) => v, Err(r) => return r };
    let fields = if v["fields"].is_object() { v["fields"].clone() } else { json!({}) };
    let prompt = if v["style"].as_str() == Some("full") {
        fixer::build_prompt(&fields, &stack_of(&v))
    } else {
        let rule = fields["ruleId"].as_str().and_then(kb::get_rule);
        fixer::build_brief_prompt(&fields, rule)
    };
    json_ok(json!({ "prompt": prompt }))
}

async fn api_patch(body: Bytes) -> Response {
    let v = match parse_body(&body) { Ok(v) => v, Err(r) => return r };
    let items = v["items"].as_array().cloned().unwrap_or_default();
    text_ok(fixer::build_patch(&items), "text/plain; charset=utf-8")
}

async fn api_report(body: Bytes) -> Response {
    let v = match parse_body(&body) { Ok(v) => v, Err(r) => return r };
    let result = fixer::build_deliverables(&finding_of(&v), &stack_of(&v));
    text_ok(fixer::build_report_md(&result), "text/markdown; charset=utf-8")
}

// ---- AI ----
async fn ai_config_get() -> Response {
    json_ok(json!({ "config": crate::ai::public_config(), "providers": crate::ai::providers() }))
}

async fn ai_config_post(body: Bytes) -> Response {
    let v = match parse_body(&body) { Ok(v) => v, Err(r) => return r };
    if let Some(b) = v["baseUrl"].as_str() {
        let b = b.trim();
        if !b.is_empty() && !b.starts_with("http://") && !b.starts_with("https://") {
            return json_err(StatusCode::BAD_REQUEST, "API 地址必须以 http:// 或 https:// 开头");
        }
    }
    crate::ai::save_config(|c| {
        if let Some(b) = v["enabled"].as_bool() { c.enabled = b; }
        if let Some(p) = v["provider"].as_str() { c.provider = p.to_string(); }
        if let Some(b) = v["baseUrl"].as_str() { c.base_url = b.trim().to_string(); }
        if let Some(m) = v["model"].as_str() { c.model = m.trim().to_string(); }
        if let Some(k) = v["apiKey"].as_str() {
            if !k.trim().is_empty() { c.api_key = k.trim().to_string(); }
        }
        if v["clearKey"].as_bool() == Some(true) { c.api_key = String::new(); }
    });
    json_ok(json!({ "config": crate::ai::public_config() }))
}

async fn ai_test(State(state): State<Arc<AppState>>) -> Response {
    match crate::ai::test_connection(&state.manual).await {
        Ok(r) => json_ok(r),
        Err(e) => json_err(StatusCode::BAD_GATEWAY, e),
    }
}

async fn ai_models(State(state): State<Arc<AppState>>, body: Bytes) -> Response {
    let v = match parse_body(&body) { Ok(v) => v, Err(r) => return r };
    let mut cfg = crate::ai::load_config();
    let base = v["baseUrl"].as_str().unwrap_or("").trim();
    if !base.is_empty() { cfg.base_url = base.to_string(); }
    let key = v["apiKey"].as_str().unwrap_or("").trim();
    if !key.is_empty() { cfg.api_key = key.to_string(); }
    match crate::ai::list_models(&state.manual, &cfg).await {
        Ok(models) => json_ok(json!({ "models": models })),
        Err(e) => json_err(StatusCode::BAD_GATEWAY, e),
    }
}

async fn ai_analyze(State(state): State<Arc<AppState>>, body: Bytes) -> Response {
    let v = match parse_body(&body) { Ok(v) => v, Err(r) => return r };
    match crate::ai::analyze(&state.manual, &v).await {
        Ok(r) => json_ok(r),
        Err(e) => json_err(StatusCode::BAD_GATEWAY, e),
    }
}

// ---- 扩展中心 ----
async fn ext_meta() -> Response {
    json_ok(json!({
        "types": extensions::TYPES,
        "transports": extensions::TRANSPORTS,
        "permissions": extensions::PERMISSIONS,
        "skillsDir": extensions::skills_dir().to_string_lossy(),
        "modes": orchestrator::MODES,
        "ops": orchestrator::OPS,
    }))
}

async fn ext_register(body: Bytes) -> Response {
    let v = match parse_body(&body) { Ok(v) => v, Err(r) => return r };
    match register_ext(&v).await {
        Ok(ext) => json_ok(json!({ "ext": ext })),
        Err(e) => json_err(StatusCode::BAD_REQUEST, e),
    }
}

async fn ext_update(body: Bytes) -> Response {
    let v = match parse_body(&body) { Ok(v) => v, Err(r) => return r };
    let id = v["id"].as_str().unwrap_or("");
    match update_ext(id, &v).await {
        Ok(ext) => json_ok(json!({ "ext": ext })),
        Err(e) => json_err(StatusCode::BAD_REQUEST, e),
    }
}

async fn ext_remove(body: Bytes) -> Response {
    let v = match parse_body(&body) { Ok(v) => v, Err(r) => return r };
    match remove_ext(v["id"].as_str().unwrap_or("")).await {
        Ok(r) => json_ok(r),
        Err(e) => json_err(StatusCode::BAD_REQUEST, e),
    }
}

async fn ext_discover(State(state): State<Arc<AppState>>, body: Bytes) -> Response {
    let v = match parse_body(&body) { Ok(v) => v, Err(r) => return r };
    match discover_ext(&state, v["id"].as_str().unwrap_or("")).await {
        Ok(r) => json_ok(r),
        Err(e) => json_err(StatusCode::BAD_GATEWAY, e),
    }
}

async fn ext_invoke(State(state): State<Arc<AppState>>, body: Bytes) -> Response {
    let v = match parse_body(&body) { Ok(v) => v, Err(r) => return r };
    let url = v["input"]["url"].as_str().filter(|u| !u.is_empty())
        .or_else(|| v["ctx"]["url"].as_str().filter(|u| !u.is_empty()));
    if let Some(e) = target_auth_error(url, v["authorized"].as_bool().unwrap_or(false)) {
        return json_err(StatusCode::BAD_REQUEST, e);
    }
    let input = if v["input"].is_object() { v["input"].clone() } else { json!({}) };
    let ctx = if v["ctx"].is_object() { v["ctx"].clone() } else { json!({}) };
    match invoke_ext(&state, v["id"].as_str().unwrap_or(""), input, ctx).await {
        Ok(r) => json_ok(r),
        Err(e) => json_err(StatusCode::BAD_GATEWAY, e),
    }
}

async fn api_audit(Query(q): Query<HashMap<String, String>>) -> Response {
    let limit = q.get("limit").and_then(|s| s.parse::<usize>().ok()).unwrap_or(100).min(500);
    json_ok(json!({ "entries": read_audit(limit) }))
}

// ---- 协同编排 ----
async fn orch_pipeline_save(body: Bytes) -> Response {
    let v = match parse_body(&body) { Ok(v) => v, Err(r) => return r };
    match orchestrator::save_pipeline(&v) {
        Ok(p) => json_ok(json!({ "pipeline": p })),
        Err(e) => json_err(StatusCode::BAD_REQUEST, e),
    }
}

async fn orch_pipeline_remove(body: Bytes) -> Response {
    let v = match parse_body(&body) { Ok(v) => v, Err(r) => return r };
    match orchestrator::remove_pipeline(v["id"].as_str().unwrap_or("")) {
        Ok(r) => json_ok(r),
        Err(e) => json_err(StatusCode::BAD_REQUEST, e),
    }
}

async fn orch_run(State(state): State<Arc<AppState>>, body: Bytes) -> Response {
    let v = match parse_body(&body) { Ok(v) => v, Err(r) => return r };
    let url = v["url"].as_str().filter(|u| !u.is_empty())
        .or_else(|| v["input"]["url"].as_str().filter(|u| !u.is_empty()));
    if let Some(e) = target_auth_error(url, v["authorized"].as_bool().unwrap_or(false)) {
        return json_err(StatusCode::BAD_REQUEST, e);
    }
    let pipeline = if v["pipeline"].is_object() { Some(v["pipeline"].clone()) } else { None };
    match orchestrator::start_pipeline(
        &state,
        v["pipelineId"].as_str().filter(|s| !s.is_empty()),
        pipeline.as_ref(),
        url.unwrap_or(""),
    ).await {
        Ok(id) => json_ok(json!({ "id": id })),
        Err(e) => json_err(StatusCode::BAD_REQUEST, e),
    }
}

async fn orch_run_status(State(state): State<Arc<AppState>>, Query(q): Query<HashMap<String, String>>) -> Response {
    match orchestrator::get_run(&state, q.get("id").map(|s| s.as_str()).unwrap_or("")).await {
        Some(r) => json_ok(r),
        None => json_err(StatusCode::NOT_FOUND, "运行不存在"),
    }
}

// ---- 静态资源（SPA + capture-hook.js / ai-pilot.js）----
async fn static_handler(req: axum::extract::Request) -> Response {
    let method = req.method().clone();
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD {
        return json_err(StatusCode::METHOD_NOT_ALLOWED, "method not allowed");
    }
    let path = req.uri().path();
    let rel = if path == "/" { "index.html" } else { path.trim_start_matches('/') };
    if rel.split('/').any(|seg| seg == "..") {
        return json_err(StatusCode::FORBIDDEN, "forbidden");
    }
    let Some(file) = Assets::get(rel) else {
        return json_err(StatusCode::NOT_FOUND, "not found");
    };
    let mime = mime_guess::from_path(rel).first_raw().unwrap_or("application/octet-stream");
    let mime = if mime.starts_with("text/") || mime == "application/javascript" {
        format!("{}; charset=utf-8", mime)
    } else {
        mime.to_string()
    };
    let data = file.data.to_vec();
    let len = data.len();
    let mut resp = (
        StatusCode::OK,
        [(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, "no-store".to_string())],
        if method == axum::http::Method::HEAD { Body::empty() } else { Body::from(data) },
    ).into_response();
    if method == axum::http::Method::HEAD {
        resp.headers_mut().insert(header::CONTENT_LENGTH, len.into());
    }
    resp
}