// 扩展内核：统一 agent / skill / mcp / bot 的注册、配置、发现、调用、权限与审计
// 传输：builtin / http / openai / module / stdio；配置持久化 ~/.webfix-studio/extensions.json
use crate::ai::{analyze, chat, load_config, ChatOpts};
use crate::crawler::{job_json, start_crawl};
use crate::kb::{get_rule, summarize};
use crate::scanner::{scan_headers, scan_html, scan_target, ScanOpts};
use crate::safety::{check_command, check_script_source, deny_entry};
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
pub const TRANSPORTS: [&str; 6] = ["builtin", "http", "openai", "module", "stdio", "cli"];
pub const PERMISSIONS: [&str; 6] = ["scan", "network", "llm", "read_html", "mcp", "notify"];

fn required(type_: &str, transport: &str) -> Vec<&'static str> {
    match (type_, transport) {
        ("agent", "builtin") => vec!["scan"],
        ("agent", "http") => vec!["network"],
        ("agent", "openai") => vec!["llm"],
        ("agent", "cli") => vec!["scan", "network"],
        ("skill", "builtin") => vec!["read_html"],
        ("skill", "http") => vec!["network"],
        ("skill", "module") => vec!["read_html"],
        ("skill", "cli") => vec!["network"],
        ("mcp", "stdio") => vec!["mcp"],
        ("mcp", "http") => vec!["mcp", "network"],
        ("mcp", "cli") => vec!["mcp", "network"],
        ("bot", "builtin") => vec!["notify"],
        ("bot", "http") => vec!["network", "notify"],
        ("bot", "cli") => vec!["network", "notify"],
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
        { "id": "agent-inspect", "type": "agent", "transport": "builtin", "name": "人工巡检结果采集", "version": "1.0.0", "description": "读取指定巡检会话已有的抓取结果；不自动启动浏览器", "permissions": ["scan"], "config": {} },
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

/// 命中安全红线：写审计后返回错误（拒绝执行 / 注册 / 导入）
fn safety_deny(target: &str, e: String) -> String {
    audit(deny_entry(target, &e));
    e
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
        if let Ok(content) = std::fs::read_to_string(&p) {
            if let Err(e) = check_script_source(&content) {
                return Err(safety_deny(&s(input, "module"), e));
            }
        }
    }
    if transport == "stdio" || transport == "cli" {
        let command = s(input, "command");
        if command.is_empty() {
            return Err("stdio/cli 传输需提供 command".into());
        }
        let args: Vec<String> = input["args"].as_array().map(|a| {
            a.iter().filter_map(|x| x.as_str().map(String::from)).collect()
        }).unwrap_or_default();
        if let Err(e) = check_command(&command, &args) {
            return Err(safety_deny(&command, e));
        }
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
    {
        let transport = s(e, "transport");
        if transport == "stdio" || transport == "cli" {
            let command = s(e, "command");
            let args: Vec<String> = e["args"].as_array().map(|a| {
                a.iter().filter_map(|x| x.as_str().map(String::from)).collect()
            }).unwrap_or_default();
            if let Err(err) = check_command(&command, &args) {
                return Err(safety_deny(&command, err));
            }
        }
        if transport == "module" {
            let p = resolve_path(&s(e, "module"));
            if let Ok(content) = std::fs::read_to_string(&p) {
                if let Err(err) = check_script_source(&content) {
                    return Err(safety_deny(&s(e, "module"), err));
                }
            }
        }
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
    if let Err(e) = check_command(&command, &args) { return Err(safety_deny(&command, e)); }
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
        "clientInfo": { "name": "webfix-studio", "version": crate::util::VERSION }
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
    if transport == "cli" {
        let out = run_cli(ext, &input, &ctx).await?;
        let findings = if out["findings"].is_array() { out["findings"].clone() } else { json!([]) };
        return Ok(json!({ "output": out, "findings": findings }));
    }
    Err(format!("未实现的传输：{}:{}", type_, transport))
}

// cli 传输：本机命令行工具（nuclei/sqlmap/semgrep 等），JSON stdin → stdout（JSON 或纯文本）
async fn run_cli(ext: &Value, input: &Value, ctx: &Value) -> Result<Value, String> {
    let command = s(ext, "command");
    let mut args: Vec<String> = ext["args"].as_array().map(|a| {
        a.iter().filter_map(|x| x.as_str().map(String::from)).collect()
    }).unwrap_or_default();
    let url = {
        let u = s(input, "url");
        if u.is_empty() { s(ctx, "url") } else { u }
    };
    // 模板替换：args 中的 {url} 替换为目标地址，便于适配各类 CLI
    for a in args.iter_mut() {
        if a.contains("{url}") { *a = a.replace("{url}", &url); }
    }
    if let Err(e) = check_command(&command, &args) { return Err(safety_deny(&command, e)); }
    let payload = json!({ "input": input, "ctx": ctx }).to_string();
    let mut child = tokio::process::Command::new(&command)
        .args(&args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("无法启动命令 {}（请确认已安装该开源工具并加入 PATH）：{}", command, e))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(payload.as_bytes()).await;
        let _ = stdin.shutdown().await;
    }
    let out = tokio::time::timeout(Duration::from_secs(120), child.wait_with_output()).await
        .map_err(|_| "cli 工具执行超时（120s）".to_string())?
        .map_err(|e| format!("cli 工具执行失败：{}", e))?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    if !out.status.success() && text.trim().is_empty() {
        return Err(format!("cli 工具退出码 {:?}：{}", out.status.code(), trunc(err.trim(), 300)));
    }
    // 优先解析 JSON（{output,findings} 或 findings 数组），否则按纯文本输出
    if let Ok(v) = serde_json::from_str::<Value>(text.trim()) {
        if v.is_object() { return Ok(v); }
        if v.is_array() { return Ok(json!({ "output": { "raw": v }, "findings": [] })); }
    }
    Ok(json!({ "output": { "stdout": trunc(text.trim(), 20000), "stderr": trunc(err.trim(), 2000) }, "findings": [] }))
}

// module 技能：本地脚本（.js/.mjs 用 Node 执行，.py 用 Python 执行），JSON stdin → JSON stdout
async fn run_module_skill(path: &Path, input: &Value, ctx: &Value) -> Result<Value, String> {
    if let Ok(content) = std::fs::read_to_string(path) {
        if let Err(e) = check_script_source(&content) {
            return Err(safety_deny(&path.display().to_string(), e));
        }
    }
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
    if id == "agent-inspect" {
        let sid = input["sid"].as_str().filter(|s| !s.is_empty())
            .or_else(|| ctx["inspectSid"].as_str().filter(|s| !s.is_empty()))
            .ok_or("请提供已有人工巡检会话 ID；该步骤不会自动打开巡检浏览器")?;
        let sessions = state.sessions.lock().await;
        let session = sessions.get(sid).ok_or("人工巡检会话不存在或已失效")?;
        let target = session.target.as_deref().and_then(|s| url::Url::parse(s).ok()).ok_or("巡检会话目标无效")?;
        let requested = url::Url::parse(&url).map_err(|_| "编排目标 URL 无效")?;
        if target.origin() != requested.origin() { return Err("巡检会话与编排目标必须同源".into()); }
        let mut findings = Vec::new();
        for event in &session.events {
            let real_url = event["realUrl"].as_str().unwrap_or("");
            if !real_url.is_empty() && url::Url::parse(real_url).map(|u| u.origin() != requested.origin()).unwrap_or(true) { continue; }
            let page = if real_url.is_empty() { url.as_str() } else { real_url };
            if event["type"] == "scan" {
                for f in event["findings"].as_array().into_iter().flatten() {
                    let mut f = f.clone(); f["url"] = json!(page); findings.push(f);
                }
            }
            let rule_id = match event["type"].as_str().unwrap_or("") {
                "exception" => Some("func-js-exception"),
                "network" => Some("func-network-error"),
                "resource" => Some("func-resource-error"),
                "console" if event["level"] == "error" => Some("func-console-error"),
                _ => None,
            };
            if let Some(rule_id) = rule_id {
                let raw = json!({ "ruleId": rule_id, "url": page, "evidence": event["message"].as_str().unwrap_or("") });
                findings.extend(crate::kb::enrich_findings(&[raw]));
            }
            if event["type"] == "dom" {
                for issue in event["issues"].as_array().into_iter().flatten() {
                    let raw = json!({ "ruleId": issue["ruleId"], "url": page, "evidence": issue["evidence"] });
                    findings.extend(crate::kb::enrich_findings(&[raw]));
                }
            }
        }
        return Ok(json!({ "output": { "events": session.events.len(), "mode": "已有巡检会话" }, "findings": findings }));
    }
    let target = url::Url::parse(&url).map_err(|_| "目标 URL 无效")?;
    let auth_input = json!({ "useSession": ctx["useSession"].as_bool().unwrap_or(true) });
    let (follow, manual) = crate::server::auth_clients(state, &auth_input, &target).await;
    if id == "agent-rule" {
        let r = scan_target(&follow, &manual, &url, ScanOpts {
            probe_paths: input["probePaths"].as_bool().unwrap_or(false),
            discover: false,
            active: input["active"].as_bool().unwrap_or(true),
            assets: input["assets"].as_bool().unwrap_or(true),
        }).await?;
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
        let job = start_crawl(follow, manual, &url, Some(max_pages), None);
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

// ---- 开源安全扩展市场：内置增强技能（零成本开箱即用）+ 开源工具适配（需本机安装）----

/// 内置增强 module 技能源码（Node ESM，default(input,ctx) → {output,findings}）
fn module_source(id: &str) -> Option<&'static str> {
    Some(match id {
        "skill-jwt" => r#"
const F = (title, severity, rootCause, evidence, url) => ({ title, category: 'security', severity, rootCause, evidence, url, source: 'skill', ruleId: null });
export default function (input = {}, ctx = {}) {
  const url = input.url || ctx.url || '';
  const hay = [input.html || '', JSON.stringify(ctx.headers || {}), JSON.stringify(input.headers || {})].join('\n');
  const tokens = [...new Set((hay.match(/eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]*/g) || []))];
  const findings = []; const decoded = [];
  for (const t of tokens.slice(0, 10)) {
    try {
      const [h, p] = t.split('.');
      const dec = (x) => JSON.parse(Buffer.from(x.replace(/-/g,'+').replace(/_/g,'/'), 'base64').toString('utf8'));
      const head = dec(h), payload = dec(p); decoded.push({ head, payload });
      if (!head.alg || String(head.alg).toLowerCase() === 'none') findings.push(F('JWT 使用 none 算法', 'critical', '签名算法为 none，可伪造任意身份', 'alg=' + head.alg, url));
      if (!payload.exp) findings.push(F('JWT 缺少过期时间 exp', 'high', '令牌永不过期，泄露后长期有效', 'payload 无 exp 字段', url));
      if (payload.exp && payload.exp * 1000 - Date.now() > 30 * 864e5) findings.push(F('JWT 有效期过长', 'medium', 'exp 超过 30 天，泄露风险窗口大', 'exp=' + payload.exp, url));
      if (/(password|passwd|secret|api_?key)/i.test(JSON.stringify(payload))) findings.push(F('JWT 载荷含敏感字段', 'high', '令牌中携带密码/密钥等敏感信息', 'payload 含敏感键', url));
    } catch (e) { /* 非标准 JWT 跳过 */ }
  }
  return { output: { checked: 'jwt', tokens: tokens.length, decoded }, findings };
}
"#,
        "skill-entropy" => r#"
const F = (title, severity, rootCause, evidence, url) => ({ title, category: 'security', severity, rootCause, evidence, url, source: 'skill', ruleId: null });
const entropy = (s) => { const m = {}; for (const c of s) m[c] = (m[c]||0)+1; return -Object.values(m).reduce((a,n)=>{const p=n/s.length;return a+p*Math.log2(p);},0); };
export default function (input = {}, ctx = {}) {
  const url = input.url || ctx.url || ''; const html = input.html || ctx.html || '';
  const findings = []; const hits = [];
  const patterns = [
    [/AKIA[0-9A-Z]{16}/g, 'AWS Access Key', 'critical'],
    [/AIza[0-9A-Za-z_-]{35}/g, 'Google API Key', 'high'],
    [/sk-[A-Za-z0-9]{20,}/g, 'OpenAI/Stripe Secret Key', 'critical'],
    [/ghp_[A-Za-z0-9]{36}/g, 'GitHub PAT', 'critical'],
    [/xox[baprs]-[A-Za-z0-9-]{10,}/g, 'Slack Token', 'high'],
    [/-----BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY-----/g, '私钥文件', 'critical'],
  ];
  for (const [re, name, sev] of patterns) { const m = html.match(re); if (m) { hits.push({ name, count: m.length }); findings.push(F('硬编码 ' + name, sev, '源码/页面中泄露 ' + name, name + ' × ' + m.length, url)); } }
  const cand = html.match(/["'][A-Za-z0-9+/=_-]{32,}["']/g) || [];
  for (const c of cand.slice(0, 200)) { const v = c.slice(1,-1); if (entropy(v) > 4.2 && !/^[a-f0-9]+$/i.test(v)) { hits.push({ name: '高熵字符串', sample: v.slice(0,12)+'…' }); findings.push(F('疑似高熵密钥/令牌', 'medium', '香农熵 > 4.2 的长随机串，可能为密钥', v.slice(0,16)+'…', url)); break; } }
  return { output: { checked: 'entropy', hits }, findings: findings.slice(0, 12) };
}
"#,
        "skill-cors" => r#"
const F = (title, severity, rootCause, evidence, url) => ({ title, category: 'security', severity, rootCause, evidence, url, source: 'skill', ruleId: null });
export default function (input = {}, ctx = {}) {
  const url = input.url || ctx.url || '';
  const h = input.headers || ctx.headers || {}; const lower = {}; for (const k in h) lower[k.toLowerCase()] = String(h[k]);
  const findings = []; const acao = lower['access-control-allow-origin'], acac = lower['access-control-allow-credentials'];
  if (acao === '*' && acac === 'true') findings.push(F('CORS 通配符 + 允许凭证', 'high', 'ACAO:* 与 ACAC:true 组合配置错误，浏览器会拒绝但反映配置混乱', 'ACAO=* ACAC=true', url));
  if (acao === '*') findings.push(F('CORS 允许任意来源', 'medium', 'Access-Control-Allow-Origin 为 *，敏感接口不应放开', 'ACAO=*', url));
  if (acao === 'null') findings.push(F('CORS 允许 null 来源', 'high', '沙箱 iframe / data URL 可发起带凭证请求', 'ACAO=null', url));
  if (acac === 'true' && acao && acao !== '*') findings.push(F('CORS 反射来源且允许凭证', 'medium', '动态反射 Origin + 允许凭证，需校验白名单', 'ACAO=' + acao + ' ACAC=true', url));
  return { output: { checked: 'cors', acao: acao || null, acac: acac || null }, findings };
}
"#,
        "skill-cookie" => r#"
const F = (title, severity, rootCause, evidence, url) => ({ title, category: 'security', severity, rootCause, evidence, url, source: 'skill', ruleId: null });
export default function (input = {}, ctx = {}) {
  const url = input.url || ctx.url || '';
  const h = input.headers || ctx.headers || {}; let raw = '';
  for (const k in h) if (k.toLowerCase() === 'set-cookie') raw = String(h[k]);
  const cookies = input.cookies || (raw ? raw.split(/,(?=[^;]+=)/) : []);
  const findings = []; const checked = [];
  for (const c of cookies.slice(0, 20)) {
    const name = (c.split('=')[0]||'').trim(); checked.push(name);
    if (!/httponly/i.test(c)) findings.push(F('Cookie 缺少 HttpOnly：' + name, 'medium', '脚本可读取 Cookie，XSS 可窃取会话', name, url));
    if (!/secure/i.test(c)) findings.push(F('Cookie 缺少 Secure：' + name, 'medium', 'Cookie 可经明文 HTTP 传输', name, url));
    if (!/samesite/i.test(c)) findings.push(F('Cookie 缺少 SameSite：' + name, 'low', '易受 CSRF 跨站携带', name, url));
  }
  return { output: { checked: 'cookie', cookies: checked }, findings: findings.slice(0, 15) };
}
"#,
        "skill-sqli-pro" => r#"
const F = (title, severity, rootCause, evidence, url) => ({ title, category: 'security', severity, rootCause, evidence, url, source: 'skill', ruleId: null });
export default function (input = {}, ctx = {}) {
  const url = input.url || ctx.url || ''; const html = input.html || ctx.html || '';
  const params = [...new Set([...(html.match(/[?&]([a-zA-Z_][\w]{0,30})=/g)||[]).map(s=>s.slice(1,-1)), ...(html.match(/name=["']([\w]{1,30})["']/g)||[]).map(s=>s.replace(/name=["']/,'').replace(/["']$/,''))])];
  const payloads = ["' OR '1'='1", "' OR 1=1-- -", "1; WAITFOR DELAY '0:0:5'--", "' AND SLEEP(5)-- -", "1 UNION SELECT NULL,NULL-- -", "admin'--", "' AND extractvalue(1,concat(0x7e,version()))-- -"];
  const templates = params.slice(0, 20).map(p => ({ param: p, tests: payloads.map(x => p + '=' + encodeURIComponent(x)) }));
  const findings = params.length ? [F('发现 ' + params.length + ' 个可注入参数面', 'medium', '存在用户可控参数，需逐一验证注入/盲注/注释截断', params.slice(0,10).join(', '), url)] : [];
  return { output: { checked: 'sqli', params, payloads, templates }, findings };
}
"#,
        "skill-idor-pro" => r#"
const F = (title, severity, rootCause, evidence, url) => ({ title, category: 'security', severity, rootCause, evidence, url, source: 'skill', ruleId: null });
export default function (input = {}, ctx = {}) {
  const url = input.url || ctx.url || ''; const html = input.html || ctx.html || '';
  const ids = [...new Set((html.match(/[?&/](id|uid|user_?id|order_?id|account|file|doc|invoice|pid)=([\w-]{1,40})/gi)||[]))];
  const paths = [...new Set((html.match(/\/(api|user|order|account|admin|export|download)\/[\w/-]{1,60}/gi)||[]))];
  const plan = ids.slice(0,20).map(m => ({ surface: m, tests: ['替换为相邻数字 ID（水平越权）', '替换为其他租户 ID（跨租户）', '未登录直接请求（垂直越权）', '修改 method 为 PUT/DELETE（功能级越权）'] }));
  const findings = (ids.length || paths.length) ? [F('发现 ' + (ids.length+paths.length) + ' 处越权测试面', 'medium', '存在对象引用/敏感路径，需验证水平/垂直/跨租户越权', [...ids,...paths].slice(0,8).join(' | '), url)] : [];
  return { output: { checked: 'idor', ids, paths, plan }, findings };
}
"#,
        _ => return None,
    })
}

/// 扩展市场目录（不自动注册，由用户一键接入）
pub fn presets() -> Vec<Value> {
    let mut v: Vec<Value> = json!([
        { "id": "skill-jwt", "type": "skill", "transport": "module", "name": "JWT 安全分析技能", "version": "1.0.0", "category": "内置增强 · 零成本", "description": "解码页面/响应头中的 JWT，检测 none 算法、缺 exp、超长有效期、敏感载荷", "permissions": ["read_html"], "enabled": true, "homepage": "", "installHint": "开箱即用，无需安装" },
        { "id": "skill-entropy", "type": "skill", "transport": "module", "name": "高熵秘钥泄露扫描技能", "version": "1.0.0", "category": "内置增强 · 零成本", "description": "识别 AWS/Google/OpenAI/GitHub/Slack 密钥、私钥与高熵随机串", "permissions": ["read_html"], "enabled": true, "homepage": "", "installHint": "开箱即用，无需安装" },
        { "id": "skill-cors", "type": "skill", "transport": "module", "name": "CORS 配置审计技能", "version": "1.0.0", "category": "内置增强 · 零成本", "description": "检测 ACAO 通配符/null/反射来源 + 允许凭证等跨域配置错误", "permissions": ["read_html"], "enabled": true, "homepage": "", "installHint": "开箱即用，无需安装" },
        { "id": "skill-cookie", "type": "skill", "transport": "module", "name": "Cookie 安全审计技能", "version": "1.0.0", "category": "内置增强 · 零成本", "description": "检查 Set-Cookie 是否缺少 HttpOnly / Secure / SameSite", "permissions": ["read_html"], "enabled": true, "homepage": "", "installHint": "开箱即用，无需安装" },
        { "id": "skill-sqli-pro", "type": "skill", "transport": "module", "name": "SQL 注入载荷生成技能", "version": "1.0.0", "category": "内置增强 · 零成本", "description": "提取参数面并生成注入/盲注/注释截断/报错注入测试载荷", "permissions": ["read_html"], "enabled": true, "homepage": "", "installHint": "开箱即用，无需安装" },
        { "id": "skill-idor-pro", "type": "skill", "transport": "module", "name": "越权（IDOR）枚举技能", "version": "1.0.0", "category": "内置增强 · 零成本", "description": "提取对象引用与敏感路径，生成水平/垂直/跨租户越权测试计划", "permissions": ["read_html"], "enabled": true, "homepage": "", "installHint": "开箱即用，无需安装" },
        { "id": "tool-nuclei", "type": "agent", "transport": "cli", "name": "Nuclei 模板漏扫", "version": "3.x", "category": "开源工具 · 需安装", "description": "ProjectDiscovery Nuclei，基于海量 YAML 模板的漏洞扫描器", "permissions": ["scan", "network"], "enabled": false, "command": "nuclei", "args": ["-u", "{url}", "-jsonl", "-silent"], "homepage": "https://github.com/projectdiscovery/nuclei", "installHint": "安装：go install -v github.com/projectdiscovery/nuclei/v3/cmd/nuclei@latest，或下载 release 加入 PATH" },
        { "id": "tool-sqlmap", "type": "agent", "transport": "cli", "name": "sqlmap 注入检测", "version": "1.x", "category": "开源工具 · 需安装", "description": "自动化 SQL 注入与数据库接管检测", "permissions": ["scan", "network"], "enabled": false, "command": "sqlmap", "args": ["-u", "{url}", "--batch", "--output-dir=/tmp/sqlmap"], "homepage": "https://github.com/sqlmapproject/sqlmap", "installHint": "安装：pip install sqlmap，或 git clone 后将 sqlmap.py 加入 PATH" },
        { "id": "tool-semgrep", "type": "skill", "transport": "cli", "name": "Semgrep 代码审计", "version": "1.x", "category": "开源工具 · 需安装", "description": "静态代码分析，按规则扫描源码中的安全缺陷", "permissions": ["network"], "enabled": false, "command": "semgrep", "args": ["--config", "auto", "--json", "--quiet"], "homepage": "https://github.com/semgrep/semgrep", "installHint": "安装：pip install semgrep（需对本机源码目录运行）" },
        { "id": "tool-nikto", "type": "agent", "transport": "cli", "name": "Nikto Web 扫描", "version": "2.x", "category": "开源工具 · 需安装", "description": "经典 Web 服务器漏洞与危险文件扫描", "permissions": ["scan", "network"], "enabled": false, "command": "nikto", "args": ["-h", "{url}", "-Format", "json"], "homepage": "https://github.com/sullo/nikto", "installHint": "安装：git clone https://github.com/sullo/nikto，perl nikto.pl 加入 PATH" },
        { "id": "tool-trufflehog", "type": "skill", "transport": "cli", "name": "TruffleHog 秘钥挖掘", "version": "3.x", "category": "开源工具 · 需安装", "description": "在仓库/文件中深度挖掘高熵秘钥与凭证", "permissions": ["network"], "enabled": false, "command": "trufflehog", "args": ["filesystem", ".", "--json"], "homepage": "https://github.com/trufflesecurity/trufflehog", "installHint": "安装：go install github.com/trufflesecurity/trufflehog/v3@latest" },
        { "id": "tool-gitleaks", "type": "skill", "transport": "cli", "name": "Gitleaks 秘钥扫描", "version": "8.x", "category": "开源工具 · 需安装", "description": "Git 仓库与文件中的硬编码秘钥检测", "permissions": ["network"], "enabled": false, "command": "gitleaks", "args": ["detect", "--no-git", "--report-format", "json", "--report-path", "/tmp/gitleaks.json"], "homepage": "https://github.com/gitleaks/gitleaks", "installHint": "安装：go install github.com/zricethezav/gitleaks/v8@latest" },
        { "id": "tool-subfinder", "type": "agent", "transport": "cli", "name": "Subfinder 子域枚举", "version": "2.x", "category": "开源工具 · 需安装", "description": "被动子域名发现，扩大攻击面巡检范围", "permissions": ["scan", "network"], "enabled": false, "command": "subfinder", "args": ["-d", "{url}", "-silent"], "homepage": "https://github.com/projectdiscovery/subfinder", "installHint": "安装：go install -v github.com/projectdiscovery/subfinder/v2/cmd/subfinder@latest" },
        { "id": "tool-zap", "type": "agent", "transport": "http", "name": "OWASP ZAP 代理扫描", "version": "2.x", "category": "开源工具 · 需安装", "description": "OWASP ZAP 守护进程，主动/被动 Web 漏洞扫描", "permissions": ["scan", "network"], "enabled": false, "endpoint": "http://127.0.0.1:8080", "homepage": "https://github.com/zaproxy/zaproxy", "installHint": "安装 ZAP 并以 daemon 模式启动：zap.sh -daemon -port 8080 -host 127.0.0.1" }
    ]).as_array().unwrap().clone();
    for p in v.iter_mut() {
        let id = p["id"].as_str().unwrap_or("").to_string();
        if p["transport"].as_str() == Some("module") {
            p["module"] = json!(skills_dir().join(format!("{}.mjs", id)).to_string_lossy());
        }
        if p["config"].is_null() { p["config"] = json!({}); }
        if p["args"].is_null() { p["args"] = json!([]); }
    }
    v
}

/// 一键接入市场预设：module 技能先落盘源码再注册；外部工具按配置注册（默认禁用，装好后再启用）
pub async fn install_preset(id: &str) -> Result<Value, String> {
    let p = presets().into_iter().find(|x| x["id"].as_str() == Some(id))
        .ok_or_else(|| format!("预设不存在：{}", id))?;
    if get_ext(id).is_some() {
        return Err(format!("该扩展已接入：{}", id));
    }
    if p["transport"].as_str() == Some("module") {
        let src = module_source(id).ok_or_else(|| format!("预设缺少技能源码：{}", id))?;
        if let Err(e) = check_script_source(src) { return Err(safety_deny(id, e)); }
        let dir = skills_dir();
        std::fs::create_dir_all(&dir).map_err(|e| format!("创建技能目录失败：{}", e))?;
        let path = dir.join(format!("{}.mjs", id));
        std::fs::write(&path, src).map_err(|e| format!("写入技能源码失败：{}", e))?;
    }
    let mut input = p.clone();
    input["id"] = json!(id);
    let ext = register_ext(&input).await?;
    Ok(json!({ "ext": ext, "installHint": p["installHint"], "homepage": p["homepage"] }))
}

/// 拖拽导入：.mjs/.js/.py 技能落盘注册为 module；.json 解析为 MCP/扩展配置注册
pub async fn import_ext(filename: &str, content: &str) -> Result<Value, String> {
    let name = Path::new(filename).file_name().and_then(|x| x.to_str()).unwrap_or(filename);
    let ext = Path::new(name).extension().and_then(|x| x.to_str()).unwrap_or("").to_lowercase();
    let stem = Path::new(name).file_stem().and_then(|x| x.to_str()).unwrap_or("imported").to_string();
    let safe_stem: String = stem.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
    match ext.as_str() {
        "mjs" | "js" | "cjs" | "py" => {
            if content.trim().is_empty() { return Err("文件内容为空".into()); }
            if content.len() > 512 * 1024 { return Err("技能文件过大（限 512KB）".into()); }
            if let Err(e) = check_script_source(content) { return Err(safety_deny(name, e)); }
            let dir = skills_dir();
            std::fs::create_dir_all(&dir).map_err(|e| format!("创建技能目录失败：{}", e))?;
            let id = format!("skill-{}", safe_stem);
            let fname = format!("{}.{}", id, if ext == "py" { "py" } else { "mjs" });
            let path = dir.join(&fname);
            std::fs::write(&path, content).map_err(|e| format!("写入技能失败：{}", e))?;
            let input = json!({
                "id": id, "type": "skill", "transport": "module",
                "name": format!("导入技能 · {}", stem), "version": "0.1.0",
                "description": format!("从 {} 拖拽导入的本地技能", name),
                "module": path.to_string_lossy(), "permissions": ["read_html"], "enabled": true,
            });
            let ext = register_ext(&input).await?;
            Ok(json!({ "ext": ext, "kind": "module" }))
        }
        "json" => {
            let v: Value = serde_json::from_str(content).map_err(|_| "JSON 配置解析失败".to_string())?;
            // 支持 {mcpServers:{name:{command,args}}} 或单个扩展配置
            let cfg = if v["mcpServers"].is_object() {
                let (k, val) = v["mcpServers"].as_object().unwrap().iter().next().ok_or("mcpServers 为空")?;
                let mut c = val.clone();
                if c["id"].is_null() { c["id"] = json!(format!("mcp-{}", k)); }
                if c["name"].is_null() { c["name"] = json!(k); }
                if c["type"].is_null() { c["type"] = json!("mcp"); }
                if c["transport"].is_null() { c["transport"] = json!(if c["command"].is_null() { "http" } else { "stdio" }); }
                if c["permissions"].is_null() { c["permissions"] = json!(["mcp", "network"]); }
                c
            } else { v };
            let ext = register_ext(&cfg).await?;
            Ok(json!({ "ext": ext, "kind": "config" }))
        }
        _ => Err("仅支持 .mjs/.js/.py 技能文件或 .json 扩展/MCP 配置".into()),
    }
}

fn extract_obj(content: &str) -> Option<Value> {
    let cleaned = content.replace("```json", "").replace("```", "");
    let st = cleaned.find('{')?;
    let en = cleaned.rfind('}')?;
    if en <= st { return None; }
    serde_json::from_str(&cleaned[st..=en]).ok()
}

/// AI 对话添加扩展：描述需求 → 生成扩展配置草稿（命中市场预设则直接返回；否则调用大模型）
pub async fn ai_compose_ext(client: &reqwest::Client, prompt: &str) -> Result<Value, String> {
    let prompt = prompt.trim();
    if prompt.is_empty() { return Err("请描述你想添加的扩展".into()); }
    let lower = prompt.to_lowercase();
    // 关键词命中市场预设（零成本可用）
    for p in presets() {
        let id = p["id"].as_str().unwrap_or("");
        let name = p["name"].as_str().unwrap_or("").to_lowercase();
        let key = id.trim_start_matches("tool-").trim_start_matches("skill-");
        if lower.contains(key) || lower.contains(&name) || (key == "jwt" && lower.contains("jwt")) {
            return Ok(json!({ "draft": p, "source": "preset", "explanation": format!("已匹配开源扩展「{}」，可直接接入。{}", p["name"], p["installHint"]) }));
        }
    }
    let c = load_config();
    if !c.enabled {
        return Err("未匹配到内置预设。请先在「AI 设置」启用 AI 增强，或用更具体的工具名（如 nuclei / sqlmap / jwt）".into());
    }
    let catalog: Vec<String> = presets().iter().map(|p| format!("- {}（{}）：{}", p["id"], p["type"], p["description"])).collect();
    let system = "你是 Web 安全扩展配置助手。根据用户需求生成一个扩展注册配置 JSON（只输出 JSON 对象，不要其它文字）。\n字段：type(agent/skill/mcp/bot), transport(http/openai/module/stdio/cli), name, description, endpoint(http/openai), command+args(stdio/cli), permissions(从 scan/network/llm/read_html/mcp/notify 选), config(对象)。\n优先复用以下开源预设：\n".to_string() + &catalog.join("\n");
    let r = chat(client, json!([
        { "role": "system", "content": system },
        { "role": "user", "content": trunc(prompt, 2000) },
    ]), ChatOpts { config: Some(c), max_tokens: 1200, ..Default::default() }).await?;
    let draft = extract_obj(&r.content).ok_or("模型输出无法解析为扩展配置，请重试或换模型")?;
    Ok(json!({ "draft": draft, "source": "ai", "model": r.model, "explanation": "AI 已生成扩展配置草稿，请核对后注册。" }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_valid_and_module_sources_present() {
        let ps = presets();
        assert!(ps.len() >= 12);
        for p in &ps {
            assert!(TYPES.contains(&p["type"].as_str().unwrap_or("")));
            assert!(TRANSPORTS.contains(&p["transport"].as_str().unwrap_or("")));
            for perm in p["permissions"].as_array().unwrap() {
                assert!(PERMISSIONS.contains(&perm.as_str().unwrap_or("")));
            }
            if p["transport"].as_str() == Some("module") {
                assert!(module_source(p["id"].as_str().unwrap_or("")).is_some());
                assert!(p["module"].as_str().unwrap_or("").ends_with(".mjs"));
            }
            if p["transport"].as_str() == Some("cli") {
                assert!(!p["command"].as_str().unwrap_or("").is_empty());
            }
        }
    }

    #[tokio::test]
    async fn ai_compose_ext_matches_preset_without_ai() {
        let client = reqwest::Client::new();
        let r = ai_compose_ext(&client, "帮我接入 nuclei 漏扫").await.unwrap();
        assert_eq!(r["source"], "preset");
        assert_eq!(r["draft"]["id"], "tool-nuclei");
        let r2 = ai_compose_ext(&client, "分析 jwt 令牌").await.unwrap();
        assert_eq!(r2["draft"]["id"], "skill-jwt");
    }

    #[test]
    fn extract_obj_parses_fenced_json() {
        let v = extract_obj("```json\n{\"type\":\"skill\",\"name\":\"x\"}\n```").unwrap();
        assert_eq!(v["type"], "skill");
        assert!(extract_obj("no json here").is_none());
    }

    #[test]
    fn builtin_module_sources_pass_safety() {
        for p in presets() {
            if p["transport"].as_str() == Some("module") {
                let id = p["id"].as_str().unwrap_or("");
                let src = module_source(id).unwrap();
                assert!(crate::safety::check_script_source(src).is_ok(), "内置技能 {} 被安全红线误拦", id);
            }
        }
    }

    #[test]
    fn cli_presets_pass_safety() {
        for p in presets() {
            if p["transport"].as_str() == Some("cli") {
                let id = p["id"].as_str().unwrap_or("");
                let cmd = p["command"].as_str().unwrap_or("");
                let args: Vec<String> = p["args"].as_array().unwrap().iter()
                    .filter_map(|x| x.as_str().map(String::from)).collect();
                assert!(crate::safety::check_command(cmd, &args).is_ok(), "开源工具 {} 被安全红线误拦", id);
            }
        }
    }
}