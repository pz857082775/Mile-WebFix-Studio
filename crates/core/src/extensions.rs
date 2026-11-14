// 扩展内核：统一 agent / skill / mcp / bot 的注册、配置、发现、调用、权限与审计
// 传输：builtin / http / openai / module / stdio；配置持久化 ~/.webfix-studio/extensions.json
use crate::ai::{analyze, chat, load_config, ChatOpts};
use crate::crawler::{job_json, start_crawl};
use crate::kb::{get_rule, summarize};
use crate::scanner::{scan_headers, scan_html, scan_target, ScanOpts};
use crate::util::{config_dir, now_iso, now_ms, trunc};
use crate::AppState;
use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

pub const TYPES: [&str; 4] = ["agent", "skill", "mcp", "bot"];
pub const TRANSPORTS: [&str; 5] = ["builtin", "http", "openai", "module", "stdio"];
pub const PERMISSIONS: [&str; 6] = ["scan", "network", "llm", "read_html", "mcp", "notify"];

fn required(type_: &str, transport: &str) -> Vec<&'static str> {
    match (type_, transport) {
        ("agent", "builtin") => vec!["scan"],
        ("agent", "http") => vec!["network"],
        ("agent", "openai") => vec!["llm"],
        ("skill", "builtin") => vec!["read_html"],
        ("skill", "http") => vec!["network"],
        ("skill", "module") => vec!["read_html"],
        ("mcp", "stdio") => vec!["mcp"],
        ("mcp", "http") => vec!["mcp", "network"],
        ("bot", "builtin") => vec!["notify"],
        ("bot", "http") => vec!["network", "notify"],
        _ => vec![],
    }
}

pub fn skills_dir() -> PathBuf { config_dir().join("skills") }
fn store_file() -> PathBuf { config_dir().join("extensions.json") }
fn audit_file() -> PathBuf { config_dir().join("audit.log") }

fn store_lock() -> &'static Mutex<()> {
    static L: OnceLock<Mutex<()>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(()))
}

// ---- 审计：500 条内存环 + 落盘 ----
fn audit_ring() -> &'static StdMutex<Vec<Value>> {
    static R: OnceLock<StdMutex<Vec<Value>>> = OnceLock::new();
    R.get_or_init(|| StdMutex::new(Vec::new()))
}

pub fn audit(entry: Value) -> Value {
    let mut rec = entry.as_object().cloned().unwrap_or_default();
    rec.insert("ts".into(), json!(now_iso()));
    let rec = Value::Object(rec);
    {
        let mut ring = audit_ring().lock().unwrap();
        ring.push(rec.clone());
        if ring.len() > 500 {
            ring.remove(0);
        }
    }
    let _ = std::fs::create_dir_all(config_dir());
    if let Ok(line) = serde_json::to_string(&rec) {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(audit_file()) {
            let _ = writeln!(f, "{}", line);
        }
    }
    rec
}

pub fn read_audit(limit: usize) -> Vec<Value> {
    {
        let ring = audit_ring().lock().unwrap();
        if !ring.is_empty() {
            return ring.iter().rev().take(limit).cloned().collect();
        }
    }
    let Ok(content) = std::fs::read_to_string(audit_file()) else { return vec![] };
    content.trim().lines()
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .take(limit)
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

// ---- 内置扩展与默认编排 ----
fn builtins() -> Vec<Value> {
    json!([
        { "id": "agent-rule", "type": "agent", "transport": "builtin", "name": "规则扫描智能体", "version": "1.0.0", "description": "内置 261 条规则单页体检", "permissions": ["scan"], "config": {} },
        { "id": "agent-crawl", "type": "agent", "transport": "builtin", "name": "全站爬行智能体", "version": "1.0.0", "description": "自动发现同域路由并聚合体检", "permissions": ["scan", "network"], "config": { "maxPages": 20 } },
        { "id": "agent-ai", "type": "agent", "transport": "builtin", "name": "AI 深度挖掘智能体", "version": "1.0.0", "description": "调用已配置大模型挖掘逻辑/越权/资损", "permissions": ["llm"], "config": {} },
        { "id": "skill-header", "type": "skill", "transport": "builtin", "name": "响应头审计技能", "version": "1.0.0", "description": "CSP/HSTS/Cookie/CORS 等响应头检查", "permissions": ["read_html"], "config": {} },
        { "id": "skill-secret", "type": "skill", "transport": "builtin", "name": "秘钥泄露扫描技能", "version": "1.0.0", "description": "HTML/JS 中硬编码秘钥、token、私钥识别", "permissions": ["read_html"], "config": {} },
        { "id": "skill-sqli", "type": "skill", "transport": "builtin", "name": "SQL 注入挖掘技能", "version": "1.0.0", "description": "输出注入/盲注/注释截断挖掘模板", "permissions": ["read_html"], "config": {} },
        { "id": "skill-idor", "type": "skill", "transport": "builtin", "name": "越权挖掘技能", "version": "1.0.0", "description": "输出 IDOR/水平/垂直越权挖掘模板", "permissions": ["read_html"], "config": {} },
        { "id": "bot-report", "type": "bot", "transport": "builtin", "name": "结果汇总 Bot", "version": "1.0.0", "description": "汇总协同结果生成精简报告", "permissions": ["notify"], "config": {} }
    ]).as_array().unwrap().clone()
}

pub fn default_pipeline() -> Value {
    json!({
        "id": "pipe-standard", "name": "标准协同挖掘", "mode": "serial", "builtin": true,
        "description": "规则扫描 → 秘钥/响应头技能 → 条件触发 AI 深挖 → 汇总报告",
        "steps": [
            { "key": "scan", "ext": "agent-rule" },
            { "key": "secret", "ext": "skill-secret" },
            { "key": "headers", "ext": "skill-header" },
            { "key": "deep", "ext": "agent-ai", "when": { "path": "aiEnabled", "op": "truthy" }, "onError": "continue" },
            { "key": "report", "ext": "bot-report" }
        ]
    })
}

fn defaults() -> Value {
    let exts: Vec<Value> = builtins().into_iter().map(|mut b| {
        b["enabled"] = json!(true);
        b["builtin"] = json!(true);
        b["createdAt"] = json!(now_ms());
        b
    }).collect();
    let mut pipe = default_pipeline();
    pipe["createdAt"] = json!(now_ms());
    json!({ "version": 1, "extensions": exts, "pipelines": [pipe] })
}

pub fn load_store() -> Value {
    let parsed = std::fs::read_to_string(store_file()).ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok());
    let Some(mut s) = parsed else { return defaults() };
    if !s.is_object() {
        return defaults();
    }
    let exts = s["extensions"].as_array_mut();
    if let Some(exts) = exts {
        for b in builtins() {
            let bid = b["id"].as_str().unwrap_or("");
            if !exts.iter().any(|e| e["id"].as_str() == Some(bid)) {
                let mut b = b;
                b["enabled"] = json!(true);
                b["builtin"] = json!(true);
                b["createdAt"] = json!(now_ms());
                exts.push(b);
            }
        }
    } else {
        s["extensions"] = defaults()["extensions"].clone();
    }
    if !s["pipelines"].is_array() {
        s["pipelines"] = json!([]);
    }
    let pipes = s["pipelines"].as_array_mut().unwrap();
    if !pipes.iter().any(|p| p["id"].as_str() == Some("pipe-standard")) {
        let mut pipe = default_pipeline();
        pipe["createdAt"] = json!(now_ms());
        pipes.push(pipe);
    }
    s
}

pub fn save_store(s: &Value) {
    let _ = std::fs::create_dir_all(config_dir());
    let _ = std::fs::write(store_file(), serde_json::to_string_pretty(s).unwrap_or_default());
}

pub fn get_ext(id: &str) -> Option<Value> {
    load_store()["extensions"].as_array()?
        .iter().find(|e| e["id"].as_str() == Some(id)).cloned()
}

pub(crate) fn base36(mut n: u64) -> String {
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if n == 0 { return "0".into(); }
    let mut out = Vec::new();
    while n > 0 {
        out.push(DIGITS[(n % 36) as usize]);
        n /= 36;
    }
    out.reverse();
    String::from_utf8(out).unwrap()
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

pub async fn register_ext(input: &Value) -> Result<Value, String> {
    let _g = store_lock().lock().await;
    let type_ = s(input, "type");
    if !TYPES.contains(&type_.as_str()) {
        return Err("type 必须为 agent/skill/mcp/bot".into());
    }
    let transport = {
        let t = s(input, "transport");
        if t.is_empty() { if type_ == "mcp" { "stdio".to_string() } else { "http".to_string() } } else { t }
    };
    if !TRANSPORTS.contains(&transport.as_str()) {
        return Err(format!("不支持的 transport：{}", transport));
    }
    if transport == "builtin" {
        return Err("builtin 传输仅限内置扩展".into());
    }
    let id = {
        let i = s(input, "id").trim().to_string();
        if i.is_empty() { format!("{}-{}", type_, base36(now_ms())) } else { i }
    };
    let mut store = load_store();
    let exts = store["extensions"].as_array().unwrap();
    if exts.iter().any(|e| e["id"].as_str() == Some(id.as_str())) {
        return Err(format!("扩展 id 已存在：{}", id));
    }
    if transport == "http" || transport == "openai" {
        let ep = s(input, "endpoint").to_lowercase();
        if !ep.starts_with("http://") && !ep.starts_with("https://") {
            return Err("http/openai 传输需提供 http(s) endpoint".into());
        }
    }
    if transport == "module" {
        let p = resolve_path(&s(input, "module"));
        if !p.starts_with(skills_dir()) {
            return Err(format!("module 技能必须位于 {}", skills_dir().display()));
        }
    }
    if transport == "stdio" && s(input, "command").is_empty() {
        return Err("stdio 传输需提供 command".into());
    }
    let perms: Vec<Value> = input["permissions"].as_array().map(|a| {
        a.iter().filter(|p| p.as_str().map(|x| PERMISSIONS.contains(&x)).unwrap_or(false)).cloned().collect()
    }).unwrap_or_default();
    let name = { let n = s(input, "name"); if n.is_empty() { id.clone() } else { n } };
    let version = { let v = s(input, "version"); if v.is_empty() { "0.1.0".to_string() } else { v } };
    let args = if input["args"].is_array() { input["args"].clone() } else { json!([]) };
    let config = if input["config"].is_object() { input["config"].clone() } else { json!({}) };
    let ext = json!({
        "id": id, "type": type_, "transport": transport,
        "name": trunc(&name, 60),
        "version": version,
        "description": trunc(&s(input, "description"), 200),
        "enabled": input.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
        "builtin": false,
        "endpoint": s(input, "endpoint"),
        "command": s(input, "command"),
        "args": args,
        "module": s(input, "module"),
        "config": config,
        "permissions": perms,
        "createdAt": now_ms(),
    });
    store["extensions"].as_array_mut().unwrap().push(ext.clone());
    save_store(&store);
    audit(json!({ "action": "register", "extId": id, "type": type_, "ok": true }));
    Ok(ext)
}

fn resolve_path(p: &str) -> PathBuf {
    let path = Path::new(p);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    }
}

pub async fn update_ext(id: &str, patch: &Value) -> Result<Value, String> {
    let _g = store_lock().lock().await;
    let mut store = load_store();
    let exts = store["extensions"].as_array_mut().unwrap();
    let Some(e) = exts.iter_mut().find(|x| x["id"].as_str() == Some(id)) else {
        return Err(format!("扩展不存在：{}", id));
    };
    for k in ["name", "description", "version", "endpoint", "command", "module"] {
        if let Some(v) = patch.get(k).and_then(|x| x.as_str()) {
            e[k] = json!(v);
        }
    }
    if let Some(v) = patch.get("enabled").and_then(|x| x.as_bool()) {
        e["enabled"] = json!(v);
    }
    if let Some(a) = patch.get("permissions").and_then(|x| x.as_array()) {
        e["permissions"] = json!(a.iter().filter(|p| p.as_str().map(|x| PERMISSIONS.contains(&x)).unwrap_or(false)).cloned().collect::<Vec<_>>());
    }
    if let Some(cfg) = patch.get("config").and_then(|x| x.as_object()) {
        let mut merged = e["config"].as_object().cloned().unwrap_or_default();
        for (k, v) in cfg {
            merged.insert(k.clone(), v.clone());
        }
        e["config"] = Value::Object(merged);
    }
    if let Some(a) = patch.get("args").and_then(|x| x.as_array()) {
        e["args"] = json!(a);
    }
    e["updatedAt"] = json!(now_ms());
    let out = e.clone();
    save_store(&store);
    audit(json!({ "action": "update", "extId": id, "ok": true }));
    Ok(out)
}

pub async fn remove_ext(id: &str) -> Result<Value, String> {
    let _g = store_lock().lock().await;
    let mut store = load_store();
    let exts = store["extensions"].as_array().unwrap();
    let Some(e) = exts.iter().find(|x| x["id"].as_str() == Some(id)) else {
        return Err(format!("扩展不存在：{}", id));
    };
    if e["builtin"].as_bool().unwrap_or(false) {
        return Err("内置扩展不可删除，可禁用".into());
    }
    store["extensions"] = json!(exts.iter().filter(|x| x["id"].as_str() != Some(id)).cloned().collect::<Vec<_>>());
    save_store(&store);
    audit(json!({ "action": "remove", "extId": id, "ok": true }));
    Ok(json!({ "ok": true }))
}

fn check_permission(ext: &Value) -> Result<(), String> {
    let need = required(s(ext, "type").as_str(), s(ext, "transport").as_str());
    let have: Vec<String> = ext["permissions"].as_array().map(|a| {
        a.iter().filter_map(|p| p.as_str().map(String::from)).collect()
    }).unwrap_or_default();
    let missing: Vec<&str> = need.iter().filter(|p| !have.contains(&p.to_string())).copied().collect();
    if !missing.is_empty() {
        return Err(format!("权限不足：{} 缺少 [{}]，请在扩展中心授予", s(ext, "id"), missing.join(",")));
    }
    Ok(())
}

// ---- MCP 客户端（JSON-RPC 2.0，stdio / http）----
fn mcp_request(method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params })
}

async fn mcp_http(client: &reqwest::Client, ext: &Value, method: &str, params: Value) -> Result<Value, String> {
    let r = client.post(s(ext, "endpoint"))
        .timeout(Duration::from_secs(15))
        .header("Content-Type", "application/json")
        .json(&mcp_request(method, params))
        .send().await
        .map_err(|e| format!("MCP HTTP 请求失败：{}", e))?;
    if !r.status().is_success() {
        return Err(format!("MCP HTTP {}", r.status().as_u16()));
    }
    let j: Value = r.json().await.map_err(|_| "MCP 响应不是合法 JSON".to_string())?;
    if !j["error"].is_null() {
        return Err(j["error"]["message"].as_str().unwrap_or("MCP error").to_string());
    }
    Ok(j["result"].clone())
}

async fn mcp_stdio(ext: &Value, method: &str, params: Value) -> Result<Value, String> {
    let command = s(ext, "command");
    let args: Vec<String> = ext["args"].as_array().map(|a| {
        a.iter().filter_map(|x| x.as_str().map(String::from)).collect()
    }).unwrap_or_default();
    let mut child = tokio::process::Command::new(&command)
        .args(&args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("无法启动 MCP 进程 {}：{}", command, e))?;

    let mut stdin = child.stdin.take().ok_or("MCP stdin 不可用")?;
    let stdout = child.stdout.take().ok_or("MCP stdout 不可用")?;
    let line = format!("{}\n", serde_json::to_string(&mcp_request(method, params)).unwrap());

    let result = tokio::time::timeout(Duration::from_secs(15), async {
        stdin.write_all(line.as_bytes()).await.map_err(|e| format!("MCP 写入失败：{}", e))?;
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(l)) = reader.next_line().await {
            if l.trim().is_empty() { continue; }
            if let Ok(j) = serde_json::from_str::<Value>(&l) {
                if j["id"].as_u64() == Some(1) {
                    if !j["error"].is_null() {
                        return Err(j["error"]["message"].as_str().unwrap_or("MCP error").to_string());
                    }
                    return Ok(j["result"].clone());
                }
            }
        }
        Err("MCP 进程提前退出".to_string())
    }).await;

    let _ = child.kill().await;
    match result {
        Ok(inner) => inner,
        Err(_) => Err("MCP stdio 超时".into()),
    }
}

async fn mcp_call(client: &reqwest::Client, ext: &Value, method: &str, params: Value) -> Result<Value, String> {
    if s(ext, "transport") == "http" {
        mcp_http(client, ext, method, params).await
    } else {
        mcp_stdio(ext, method, params).await
    }
}

pub async fn mcp_discover(client: &reqwest::Client, ext: &Value) -> Result<Value, String> {
    check_permission(ext)?;
    let _ = mcp_call(client, ext, "initialize", json!({
        "protocolVersion": "2024-11-05", "capabilities": {},
        "clientInfo": { "name": "webfix-studio", "version": "1.0.0" }
    })).await;
    let r = mcp_call(client, ext, "tools/list", json!({})).await?;
    let tools: Vec<Value> = r["tools"].as_array().map(|a| {
        a.iter().map(|t| json!({
            "name": t["name"].as_str().unwrap_or(""),
            "description": t["description"].as_str().unwrap_or(""),
        })).collect()
    }).unwrap_or_default();
    Ok(json!(tools))
}

pub async fn mcp_invoke_tool(client: &reqwest::Client, ext: &Value, tool: &str, args: Value) -> Result<Value, String> {
    check_permission(ext)?;
    mcp_call(client, ext, "tools/call", json!({ "name": tool, "arguments": args })).await
}

// ---- 统一调用入口 ----
pub async fn invoke_ext(state: &Arc<AppState>, id: &str, input: Value, ctx: Value) -> Result<Value, String> {
    let ext = get_ext(id).ok_or_else(|| format!("扩展不存在：{}", id))?;
    if !ext["enabled"].as_bool().unwrap_or(false) {
        return Err(format!("扩展已禁用：{}", id));
    }
    check_permission(&ext)?;
    let t0 = now_ms();
    let pipeline = ctx.get("pipelineId").cloned().unwrap_or(Value::Null);
    match dispatch(state, &ext, input, ctx).await {
        Ok(out) => {
            let ms = now_ms() - t0;
            audit(json!({ "action": "invoke", "extId": id, "type": ext["type"], "ok": true, "ms": ms, "pipeline": pipeline }));
            let mut result = json!({ "ok": true, "ms": ms });
            if let Some(o) = out.as_object() {
                for (k, v) in o {
                    result[k] = v.clone();
                }
            }
            Ok(result)
        }
        Err(e) => {
            let ms = now_ms() - t0;
            audit(json!({ "action": "invoke", "extId": id, "type": ext["type"], "ok": false, "ms": ms, "error": e, "pipeline": pipeline }));
            Err(e)
        }
    }
}

async fn dispatch(state: &Arc<AppState>, ext: &Value, input: Value, ctx: Value) -> Result<Value, String> {
    let type_ = s(ext, "type");
    let transport = s(ext, "transport");
    if type_ == "agent" && transport == "builtin" {
        return builtin_agent(state, ext, &input, &ctx).await;
    }
    if type_ == "skill" && transport == "builtin" {
        return builtin_skill(ext, &input, &ctx);
    }
    if type_ == "bot" && transport == "builtin" {
        return Ok(builtin_bot(&input, &ctx));
    }
    if type_ == "mcp" {
        let tool = {
            let t = s(&input, "tool");
            if t.is_empty() { s(&ext["config"], "tool") } else { t }
        };
        if tool.is_empty() {
            return Err("MCP 调用需指定 tool".into());
        }
        let args = if input["args"].is_null() { json!({}) } else { input["args"].clone() };
        let r = mcp_invoke_tool(&state.manual, ext, &tool, args).await?;
        return Ok(json!({ "output": r, "findings": [] }));
    }
    if transport == "http" {
        let ep = s(ext, "endpoint").trim_end_matches('/').to_string();
        let r = state.manual.post(format!("{}/invoke", ep))
            .timeout(Duration::from_secs(30))
            .header("Content-Type", "application/json")
            .json(&json!({ "id": ext["id"], "type": ext["type"], "input": input, "context": ctx }))
            .send().await
            .map_err(|e| format!("第三方扩展请求失败：{}", e))?;
        if !r.status().is_success() {
            return Err(format!("第三方扩展返回 {}", r.status().as_u16()));
        }
        let j: Value = r.json().await.map_err(|_| "第三方扩展响应不是合法 JSON".to_string())?;
        let output = if j.get("output").is_some() { j["output"].clone() } else { j.clone() };
        let findings = if j["findings"].is_array() { j["findings"].clone() } else { json!([]) };
        return Ok(json!({ "output": output, "findings": findings }));
    }
    if transport == "openai" {
        let saved = load_config();
        let mut c = saved.clone();
        let ep = s(ext, "endpoint");
        if !ep.is_empty() { c.base_url = ep; }
        let key = s(&ext["config"], "apiKey");
        if !key.is_empty() { c.api_key = key; }
        let model = s(&ext["config"], "model");
        if !model.is_empty() { c.model = model; }
        let system = {
            let sys = s(&ext["config"], "system");
            if sys.is_empty() { "你是 Web 安全审计智能体，输出 JSON 数组 findings。".to_string() } else { sys }
        };
        let r = chat(&state.follow, json!([
            { "role": "system", "content": system },
            { "role": "user", "content": trunc(&serde_json::to_string(&input).unwrap_or_default(), 12000) },
        ]), ChatOpts { config: Some(c), ..Default::default() }).await?;
        let mut findings = json!([]);
        if let (Some(st), Some(en)) = (r.content.find('['), r.content.rfind(']')) {
            if en > st {
                if let Ok(arr) = serde_json::from_str::<Value>(&r.content[st..=en]) {
                    if arr.is_array() { findings = arr; }
                }
            }
        }
        return Ok(json!({ "output": r.content, "findings": findings }));
    }
    if transport == "module" {
        let p = resolve_path(&s(ext, "module"));
        if !p.starts_with(skills_dir()) {
            return Err("module 技能路径越界".into());
        }
        let out = run_module_skill(&p, &input, &ctx).await?;
        let findings = if out["findings"].is_array() { out["findings"].clone() } else { json!([]) };
        return Ok(json!({ "output": out, "findings": findings }));
    }
    Err(format!("未实现的传输：{}:{}", type_, transport))
}

// module 技能：本地脚本（.js/.mjs 用 Node 执行，.py 用 Python 执行），JSON stdin → JSON stdout
async fn run_module_skill(path: &Path, input: &Value, ctx: &Value) -> Result<Value, String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let payload = json!({ "input": input, "ctx": ctx }).to_string();
    let (program, args): (String, Vec<String>) = match ext.as_str() {
        "js" | "mjs" | "cjs" => {
            let script = r#"
import { pathToFileURL } from 'node:url';
const p = process.argv[1];
const chunks = [];
for await (const c of process.stdin) chunks.push(c);
const { input, ctx } = JSON.parse(Buffer.concat(chunks).toString('utf8') || '{}');
const mod = await import(pathToFileURL(p).href);
const fn = mod.default || mod.invoke;
if (typeof fn !== 'function') { console.error('module 技能需导出 default(input,ctx) 函数'); process.exit(1); }
const out = await fn(input ?? {}, ctx ?? {});
process.stdout.write(JSON.stringify(out ?? null));
"#;
            ("node".into(), vec!["--input-type=module".into(), "-e".into(), script.into(), path.display().to_string()])
        }
        "py" => {
            let script = r#"
import sys, json, importlib.util
p = sys.argv[1]
spec = importlib.util.spec_from_file_location("wf_skill", p)
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)
fn = getattr(mod, "invoke", None) or getattr(mod, "default", None)
if fn is None:
    sys.stderr.write("module 技能需导出 invoke(input, ctx) 函数")
    sys.exit(1)
data = json.load(sys.stdin)
out = fn(data.get("input") or {}, data.get("ctx") or {})
print(json.dumps(out, ensure_ascii=False))
"#;
            ("python".into(), vec!["-c".into(), script.into(), path.display().to_string()])
        }
        _ => return Err("module 技能仅支持 .js/.mjs（Node）或 .py（Python）脚本".into()),
    };
    let mut child = tokio::process::Command::new(&program)
        .args(&args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("无法启动 {}（module 技能需要对应运行时）：{}", program, e))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(payload.as_bytes()).await;
        let _ = stdin.shutdown().await;
    }
    let out = child.wait_with_output().await.map_err(|e| format!("module 技能执行失败：{}", e))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("module 技能执行出错：{}", trunc(err.trim(), 300)));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(text.trim()).map_err(|_| "module 技能输出不是合法 JSON".to_string())
}

async fn builtin_agent(state: &Arc<AppState>, ext: &Value, input: &Value, ctx: &Value) -> Result<Value, String> {
    let id = s(ext, "id");
    let url = {
        let u = s(input, "url");
        if u.is_empty() { s(ctx, "url") } else { u }
    };
    if id == "agent-rule" {
        if url.is_empty() { return Err("agent-rule 需要 input.url".into()); }
        let r = scan_target(&state.follow, &state.manual, &url, ScanOpts { probe_paths: input["probePaths"].as_bool().unwrap_or(false) }).await?;
        let findings = r["findings"].as_array().cloned().unwrap_or_default();
        return Ok(json!({
            "output": { "status": r["status"], "pages": 1, "headers": r["headers"], "html": r["html"] },
            "findings": findings,
            "summary": summarize(&findings),
        }));
    }
    if id == "agent-crawl" {
        if url.is_empty() { return Err("agent-crawl 需要 input.url".into()); }
        let max_pages = ext["config"]["maxPages"].as_u64().unwrap_or(20);
        let job = start_crawl(state.follow.clone(), state.manual.clone(), &url, Some(max_pages), None);
        let job_id = job.lock().await.id.clone();
        state.crawls.lock().await.insert(job_id.clone(), job.clone());
        let t0 = now_ms();
        loop {
            let snapshot = job_json(&*job.lock().await);
            if snapshot["status"].as_str() != Some("running") {
                return Ok(json!({
                    "output": { "pages": snapshot["pages"].as_array().map(|a| a.len()).unwrap_or(0) },
                    "findings": snapshot["findings"],
                    "summary": snapshot["summary"],
                }));
            }
            if now_ms() - t0 > 60_000 { return Err("爬行超时".into()); }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
    if id == "agent-ai" {
        let kind = { let k = s(input, "kind"); if k.is_empty() { "scan".to_string() } else { k } };
        let findings = if input["findings"].is_array() { input["findings"].clone() }
            else if ctx["findings"].is_array() { ctx["findings"].clone() }
            else { json!([]) };
        let ev = json!({
            "kind": kind, "url": url, "findings": findings,
            "headers": input["headers"], "html": input["html"],
        });
        let r = analyze(&state.follow, &ev).await?;
        return Ok(json!({ "output": { "model": r["model"] }, "findings": r["findings"] }));
    }
    Err(format!("未知内置智能体：{}", id))
}

fn headers_from_value(v: &Value) -> HeaderMap {
    let mut map = HeaderMap::new();
    if let Some(obj) = v.as_object() {
        for (k, val) in obj {
            if let Some(sv) = val.as_str() {
                if let (Ok(name), Ok(hv)) = (reqwest::header::HeaderName::from_bytes(k.as_bytes()), HeaderValue::from_str(sv)) {
                    map.insert(name, hv);
                }
            }
        }
    }
    map
}

fn builtin_skill(ext: &Value, input: &Value, ctx: &Value) -> Result<Value, String> {
    let id = s(ext, "id");
    let html = {
        let h = s(input, "html");
        if h.is_empty() { s(ctx, "html") } else { h }
    };
    let url = {
        let u = s(input, "url");
        if u.is_empty() {
            let c = s(ctx, "url");
            if c.is_empty() { "http://localhost/".to_string() } else { c }
        } else { u }
    };
    if id == "skill-header" {
        let hv = if input["headers"].is_object() { &input["headers"] } else { &ctx["headers"] };
        let headers = headers_from_value(hv);
        let cookies: Vec<String> = input["cookies"].as_array().map(|a| {
            a.iter().filter_map(|c| c.as_str().map(String::from)).collect()
        }).unwrap_or_default();
        let findings = scan_headers(&headers, &url, &cookies);
        return Ok(json!({ "output": { "checked": "headers" }, "findings": findings }));
    }
    if id == "skill-secret" {
        let re = regex::Regex::new(r"(?i)secret|password|sql-error").unwrap();
        let findings: Vec<Value> = scan_html(&html, &url).into_iter()
            .filter(|f| re.is_match(f["ruleId"].as_str().unwrap_or("")))
            .collect();
        return Ok(json!({ "output": { "checked": "secret" }, "findings": findings }));
    }
    if id == "skill-sqli" || id == "skill-idor" {
        let ids: &[&str] = if id == "skill-sqli" {
            &["sec-sqli", "sec-sqli-comment-trunc", "sec-sqli-inline-waf", "sec-sqli-second-order", "sec-sqli-error-based", "sec-sqli-stacked"]
        } else {
            &["sec-idor-export", "sec-idor-file", "sec-idor-write", "sec-tenant-cross", "sec-func-level"]
        };
        let templates: Vec<Value> = ids.iter().filter_map(|i| {
            let k = get_rule(i)?;
            Some(json!({ "title": k.title, "test": k.test, "severity": k.severity }))
        }).collect();
        return Ok(json!({ "output": { "templates": templates }, "findings": [] }));
    }
    Err(format!("未知内置技能：{}", id))
}

fn builtin_bot(input: &Value, ctx: &Value) -> Value {
    let findings = if input["findings"].is_array() { input["findings"].as_array().unwrap().clone() }
        else if ctx["findings"].is_array() { ctx["findings"].as_array().unwrap().clone() }
        else { vec![] };
    let mut by_sev = Map::new();
    for f in &findings {
        let sev = f["severity"].as_str().unwrap_or("medium");
        let n = by_sev.get(sev).and_then(|v| v.as_u64()).unwrap_or(0);
        by_sev.insert(sev.into(), json!(n + 1));
    }
    let lines: Vec<String> = findings.iter().take(20).enumerate().map(|(i, f)| {
        format!("{}. [{}] {}", i + 1, f["severity"].as_str().unwrap_or(""), f["title"].as_str().unwrap_or(""))
    }).collect();
    let mut report = vec![
        format!("协同挖掘汇总：共 {} 条", findings.len()),
        format!("分级：{}", Value::Object(by_sev)),
    ];
    report.extend(lines);
    json!({ "output": { "report": report.join("\n"), "count": findings.len() }, "findings": [] })
}

/// 发现：内置目录 + 第三方 manifest / MCP tools
pub async fn discover_ext(state: &Arc<AppState>, id: &str) -> Result<Value, String> {
    let ext = get_ext(id).ok_or_else(|| format!("扩展不存在：{}", id))?;
    if s(&ext, "type") == "mcp" {
        let caps = mcp_discover(&state.manual, &ext).await?;
        return Ok(json!({ "capabilities": caps }));
    }
    if s(&ext, "transport") == "http" {
        let ep = s(&ext, "endpoint").trim_end_matches('/').to_string();
        let r = state.manual.get(format!("{}/manifest", ep))
            .timeout(Duration::from_secs(8))
            .send().await
            .map_err(|e| format!("manifest 请求失败：{}", e))?;
        if !r.status().is_success() {
            return Err(format!("manifest 返回 {}", r.status().as_u16()));
        }
        let caps: Value = r.json().await.map_err(|_| "manifest 不是合法 JSON".to_string())?;
        return Ok(json!({ "capabilities": caps }));
    }
    Ok(json!({ "capabilities": {
        "name": ext["name"], "type": ext["type"], "transport": ext["transport"], "permissions": ext["permissions"],
    } }))
}