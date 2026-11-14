// 协同编排引擎：串行 / 并行 / 条件 三种流程，统一上下文传递、超时重试、错误策略与审计。
// 条件谓词为白名单操作符 + 路径取值，不使用 eval，第三方编排定义不可执行任意代码。（与 JS orchestrator.js 对齐）
use crate::extensions::{audit, base36, invoke_ext, load_store, save_store};
use crate::kb::summarize;
use crate::util::{now_ms, trunc};
use crate::AppState;
use serde_json::{json, Map, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

pub const MODES: [&str; 3] = ["serial", "parallel", "conditional"];
pub const OPS: [&str; 9] = ["eq", "ne", "gt", "gte", "lt", "lte", "includes", "truthy", "falsy"];
const ERROR_POLICIES: [&str; 2] = ["stop", "continue"];

pub struct Run {
    pub id: String,
    pub pipeline_id: String,
    pub name: String,
    pub mode: String,
    pub status: String,
    pub started_at: u64,
    pub finished_at: Option<u64>,
    pub steps: Vec<Value>,
    pub context: Value,
    pub summary: Option<Value>,
    pub error: Option<String>,
}

pub type SharedRun = Arc<Mutex<Run>>;

// ---- 编排定义 CRUD ----
pub fn list_pipelines() -> Value {
    load_store()["pipelines"].clone()
}

fn clamp_num(v: &Value, lo: f64, hi: f64) -> f64 {
    let n = v.as_f64().unwrap_or(0.0);
    if n.is_nan() { 0.0 } else { n.clamp(lo, hi) }
}

pub fn validate_pipeline(p: &Value) -> Result<Value, String> {
    if !p.is_object() {
        return Err("编排定义格式不正确".into());
    }
    let mode = p["mode"].as_str().unwrap_or("serial");
    let mode = if mode.is_empty() { "serial" } else { mode };
    if !MODES.contains(&mode) {
        return Err("mode 必须为 serial/parallel/conditional".into());
    }
    let steps_in = p["steps"].as_array().ok_or("编排至少包含一个步骤")?;
    if steps_in.is_empty() {
        return Err("编排至少包含一个步骤".into());
    }
    if steps_in.len() > 20 {
        return Err("单编排最多 20 个步骤".into());
    }
    let mut steps = Vec::new();
    for (i, s) in steps_in.iter().enumerate() {
        let ext = s["ext"].as_str().unwrap_or("");
        if ext.is_empty() {
            return Err(format!("步骤 {} 缺少 ext（扩展 id）", i + 1));
        }
        let default_key = format!("{}-{}", ext, i + 1);
        let key = s["key"].as_str().filter(|k| !k.is_empty()).unwrap_or(&default_key);
        let mut step = json!({
            "key": trunc(key, 40),
            "ext": ext,
            "input": if s["input"].is_object() { s["input"].clone() } else { json!({}) },
            "onError": if ERROR_POLICIES.contains(&s["onError"].as_str().unwrap_or("")) { s["onError"].clone() } else { json!("continue") },
            "retry": clamp_num(&s["retry"], 0.0, 3.0) as u64,
            "timeoutMs": clamp_num(&s["timeoutMs"], 0.0, 600_000.0) as u64,
        });
        if !s["when"].is_null() {
            let path = s["when"]["path"].as_str().unwrap_or("");
            let op = s["when"]["op"].as_str().unwrap_or("");
            if path.is_empty() || !OPS.contains(&op) {
                return Err(format!("步骤 {} 条件需包含 path 与合法 op（{}）", i + 1, OPS.join("/")));
            }
            step["when"] = json!({ "path": path, "op": op, "value": s["when"]["value"] });
        }
        steps.push(step);
    }
    let id = p["id"].as_str().unwrap_or("").trim().to_string();
    let id = if id.is_empty() { format!("pipe-{}", base36(now_ms())) } else { id };
    let name = p["name"].as_str().filter(|n| !n.is_empty()).unwrap_or("未命名编排");
    Ok(json!({
        "id": id,
        "name": trunc(name, 60),
        "description": trunc(p["description"].as_str().unwrap_or(""), 200),
        "mode": mode,
        "steps": steps,
        "builtin": p["builtin"].as_bool().unwrap_or(false),
    }))
}

pub fn save_pipeline(input: &Value) -> Result<Value, String> {
    let mut p = validate_pipeline(input)?;
    let mut store = load_store();
    let pipelines = store["pipelines"].as_array().cloned().unwrap_or_default();
    let old = pipelines.iter().find(|x| x["id"] == p["id"]);
    if old.map(|o| o["builtin"].as_bool().unwrap_or(false)).unwrap_or(false)
        && !input["builtin"].as_bool().unwrap_or(false)
    {
        return Err("内置编排不可覆盖，请另存为新编排".into());
    }
    p["createdAt"] = json!(old.and_then(|o| o["createdAt"].as_u64()).unwrap_or_else(now_ms));
    p["updatedAt"] = json!(now_ms());
    let mut kept: Vec<Value> = pipelines.into_iter().filter(|x| x["id"] != p["id"]).collect();
    kept.push(p.clone());
    store["pipelines"] = json!(kept);
    save_store(&store);
    audit(json!({ "action": "pipeline-save", "pipelineId": p["id"], "ok": true }));
    Ok(p)
}

pub fn remove_pipeline(id: &str) -> Result<Value, String> {
    let mut store = load_store();
    let pipelines = store["pipelines"].as_array().cloned().unwrap_or_default();
    let p = pipelines.iter().find(|x| x["id"].as_str() == Some(id))
        .ok_or_else(|| format!("编排不存在：{}", id))?;
    if p["builtin"].as_bool().unwrap_or(false) {
        return Err("内置编排不可删除".into());
    }
    let kept: Vec<Value> = pipelines.into_iter().filter(|x| x["id"].as_str() != Some(id)).collect();
    store["pipelines"] = json!(kept);
    save_store(&store);
    audit(json!({ "action": "pipeline-remove", "pipelineId": id, "ok": true }));
    Ok(json!({ "ok": true }))
}

// ---- 条件求值（无 eval，JS 语义）----
fn get_path<'a>(obj: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = obj;
    for part in path.split('.') {
        match cur {
            Value::Object(m) => cur = m.get(part)?,
            Value::Array(a) => cur = a.get(part.parse::<usize>().ok()?)?,
            _ => return None,
        }
    }
    Some(cur)
}

fn js_truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true, // 数组 / 对象在 JS 中恒为真
    }
}

/// JS Number() 强转；None 表示 NaN
fn js_number(v: Option<&Value>) -> Option<f64> {
    match v {
        None => Some(f64::NAN),
        Some(Value::Null) => Some(0.0),
        Some(Value::Bool(b)) => Some(if *b { 1.0 } else { 0.0 }),
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => {
            let t = s.trim();
            if t.is_empty() { Some(0.0) } else { t.parse::<f64>().ok() }
        }
        Some(Value::Array(a)) if a.is_empty() => Some(0.0),
        Some(Value::Array(a)) if a.len() == 1 => js_number(Some(&a[0])),
        Some(_) => Some(f64::NAN),
    }
}

/// JS === 严格相等（对象/数组按引用比较，JSON 场景下恒不相等）
fn js_strict_eq(a: Option<&Value>, b: Option<&Value>) -> bool {
    match (a, b) {
        (None, None) | (Some(Value::Null), Some(Value::Null)) => true,
        (Some(Value::Bool(x)), Some(Value::Bool(y))) => x == y,
        (Some(Value::Number(x)), Some(Value::Number(y))) => x.as_f64() == y.as_f64(),
        (Some(Value::String(x)), Some(Value::String(y))) => x == y,
        _ => false,
    }
}

pub fn eval_cond(cond: &Value, ctx: &Value) -> bool {
    if cond.is_null() {
        return true;
    }
    let path = cond["path"].as_str().unwrap_or("");
    let left = get_path(ctx, path);
    let right = cond.get("value");
    match cond["op"].as_str().unwrap_or("") {
        "truthy" => js_truthy(left),
        "falsy" => !js_truthy(left),
        "eq" => js_strict_eq(left, right),
        "ne" => !js_strict_eq(left, right),
        "gt" | "gte" | "lt" | "lte" => {
            let l = js_number(left).unwrap_or(f64::NAN);
            let r = js_number(right).unwrap_or(f64::NAN);
            if l.is_nan() || r.is_nan() {
                return false;
            }
            match cond["op"].as_str().unwrap() {
                "gt" => l > r,
                "gte" => l >= r,
                "lt" => l < r,
                _ => l <= r,
            }
        }
        "includes" => match left {
            Some(Value::Array(a)) => a.iter().any(|x| js_strict_eq(Some(x), right)),
            _ => {
                let l = match left { None | Some(Value::Null) => String::new(), Some(v) => value_to_string(v) };
                let r = match right { None | Some(Value::Null) => String::new(), Some(v) => value_to_string(v) };
                l.contains(&r)
            }
        },
        _ => false,
    }
}

fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "null".into(),
        other => other.to_string(),
    }
}

// ---- 上下文与合并 ----
fn new_context(url: &str) -> Value {
    json!({
        "url": url,
        "findings": [],
        "stats": { "total": 0, "critical": 0, "high": 0, "medium": 0, "low": 0 },
        "outputs": {},
        "aiEnabled": crate::ai::load_config().enabled,
    })
}

fn finding_key(f: &Value) -> String {
    format!("{}|{}", f["ruleId"].as_str().unwrap_or(""), f["title"].as_str().unwrap_or(""))
}

fn merge_findings(ctx: &mut Value, list: &[Value]) {
    let findings = ctx["findings"].as_array().cloned().unwrap_or_default();
    let mut seen: std::collections::HashSet<String> = findings.iter().map(finding_key).collect();
    let mut merged = findings;
    for f in list {
        if !f.is_object() || f["title"].as_str().unwrap_or("").is_empty() {
            continue;
        }
        let key = finding_key(f);
        if seen.contains(&key) {
            continue;
        }
        seen.insert(key);
        merged.push(f.clone());
    }
    let mut stats = Map::new();
    stats.insert("total".into(), json!(merged.len()));
    for sev in ["critical", "high", "medium", "low"] {
        let n = merged.iter().filter(|f| f["severity"].as_str() == Some(sev)).count();
        stats.insert(sev.into(), json!(n));
    }
    ctx["findings"] = json!(merged);
    ctx["stats"] = Value::Object(stats);
}

struct Outcome {
    rec: Value,
    output: Value,
    findings: Vec<Value>,
}

async fn run_step(state: &Arc<AppState>, step: &Value, ctx: &Value, run_id: &str) -> Outcome {
    let key = step["key"].as_str().unwrap_or("").to_string();
    let ext = step["ext"].as_str().unwrap_or("").to_string();
    let mut rec = json!({ "key": key, "ext": ext, "status": "ok", "ms": 0, "findings": 0 });
    if !step["when"].is_null() && !eval_cond(&step["when"], ctx) {
        rec["status"] = json!("skipped");
        rec["reason"] = json!("条件不满足");
        return Outcome { rec, output: Value::Null, findings: vec![] };
    }
    let t0 = now_ms();
    let attempts = step["retry"].as_u64().unwrap_or(0) + 1;
    let timeout_ms = step["timeoutMs"].as_u64().unwrap_or(0);
    let mut last_err = String::new();
    for _ in 0..attempts {
        let mut input = step["input"].as_object().cloned().unwrap_or_default();
        let url = input.get("url").and_then(|v| v.as_str()).filter(|u| !u.is_empty())
            .map(|u| u.to_string())
            .unwrap_or_else(|| ctx["url"].as_str().unwrap_or("").to_string());
        input.insert("url".into(), json!(url));
        let mut ext_ctx = ctx.as_object().cloned().unwrap_or_default();
        ext_ctx.insert("pipelineId".into(), json!(run_id));
        let fut = invoke_ext(state, &ext, Value::Object(input), Value::Object(ext_ctx));
        let result = if timeout_ms > 0 {
            match tokio::time::timeout(Duration::from_millis(timeout_ms), fut).await {
                Ok(r) => r,
                Err(_) => Err(format!("步骤超时（{}ms）", timeout_ms)),
            }
        } else {
            fut.await
        };
        match result {
            Ok(out) => {
                rec["ms"] = json!(now_ms() - t0);
                let findings = out["findings"].as_array().cloned().unwrap_or_default();
                rec["findings"] = json!(findings.len());
                let output = out.get("output").cloned().unwrap_or(Value::Null);
                return Outcome { rec, output, findings };
            }
            Err(e) => last_err = e,
        }
    }
    rec["status"] = json!("error");
    rec["ms"] = json!(now_ms() - t0);
    rec["error"] = json!(last_err);
    Outcome { rec, output: Value::Null, findings: vec![] }
}

fn apply_outcome(run: &mut Run, outcome: Outcome) {
    let key = outcome.rec["key"].as_str().unwrap_or("").to_string();
    run.context["outputs"][key] = outcome.output.clone();
    if let Some(o) = outcome.output.as_object() {
        if let Some(h) = o.get("headers") {
            run.context["headers"] = h.clone();
        }
        if let Some(h) = o.get("html") {
            run.context["html"] = h.clone();
        }
    }
    merge_findings(&mut run.context, &outcome.findings);
    run.steps.push(outcome.rec);
}

// ---- 运行 ----
pub async fn start_pipeline(
    state: &Arc<AppState>,
    pipeline_id: Option<&str>,
    pipeline: Option<&Value>,
    url: &str,
) -> Result<String, String> {
    let def = if let Some(id) = pipeline_id {
        let stored = load_store()["pipelines"].as_array().cloned().unwrap_or_default()
            .into_iter().find(|p| p["id"].as_str() == Some(id))
            .ok_or_else(|| format!("编排不存在：{}", id))?;
        validate_pipeline(&stored)?
    } else {
        validate_pipeline(pipeline.ok_or("缺少编排定义")?)?
    };
    let rand4 = uuid::Uuid::new_v4().simple().to_string()[..4].to_string();
    let id = format!("run-{}{}", base36(now_ms()), rand4);
    let run = Arc::new(Mutex::new(Run {
        id: id.clone(),
        pipeline_id: def["id"].as_str().unwrap_or("").to_string(),
        name: def["name"].as_str().unwrap_or("").to_string(),
        mode: def["mode"].as_str().unwrap_or("serial").to_string(),
        status: "running".into(),
        started_at: now_ms(),
        finished_at: None,
        steps: vec![],
        context: new_context(url),
        summary: None,
        error: None,
    }));
    {
        let mut runs = state.runs.lock().await;
        runs.push((id.clone(), run.clone()));
        if runs.len() > 20 {
            runs.remove(0);
        }
    }

    let state2 = state.clone();
    let run2 = run.clone();
    let def2 = def.clone();
    let id2 = id.clone();
    tokio::spawn(async move {
        let t0 = now_ms();
        let steps = def2["steps"].as_array().cloned().unwrap_or_default();
        let mode = def2["mode"].as_str().unwrap_or("serial");
        if mode == "parallel" {
            let ctx_snapshot = run2.lock().await.context.clone();
            let futs: Vec<_> = steps.iter()
                .map(|s| run_step(&state2, s, &ctx_snapshot, &id2))
                .collect();
            let outcomes = futures::future::join_all(futs).await;
            let mut r = run2.lock().await;
            for o in outcomes {
                apply_outcome(&mut r, o);
            }
        } else {
            for s in &steps {
                let ctx = run2.lock().await.context.clone();
                let o = run_step(&state2, s, &ctx, &id2).await;
                let on_error = s["onError"].as_str().unwrap_or("continue");
                let failed = o.rec["status"].as_str() == Some("error");
                let err_msg = o.rec["error"].as_str().unwrap_or("").to_string();
                let key = o.rec["key"].as_str().unwrap_or("").to_string();
                let mut r = run2.lock().await;
                apply_outcome(&mut r, o);
                if failed && on_error == "stop" {
                    r.error = Some(format!("步骤 {} 失败且策略为 stop：{}", key, err_msg));
                    break;
                }
            }
        }
        let mut r = run2.lock().await;
        r.summary = Some(summarize(r.context["findings"].as_array().map(|a| a.as_slice()).unwrap_or(&[])));
        r.status = if r.error.is_some() { "error".into() } else { "done".into() };
        r.finished_at = Some(now_ms());
        audit(json!({
            "action": "pipeline-run",
            "pipelineId": def2["id"],
            "runId": id2,
            "ok": r.status == "done",
            "ms": now_ms() - t0,
            "findings": r.context["findings"].as_array().map(|a| a.len()).unwrap_or(0),
            "error": r.error,
        }));
    });

    Ok(id)
}

pub async fn get_run(state: &Arc<AppState>, id: &str) -> Option<Value> {
    let run = {
        let runs = state.runs.lock().await;
        runs.iter().find(|(rid, _)| rid == id).map(|(_, r)| r.clone())?
    };
    let r = run.lock().await;
    Some(json!({
        "id": r.id,
        "pipelineId": r.pipeline_id,
        "name": r.name,
        "mode": r.mode,
        "status": r.status,
        "startedAt": r.started_at,
        "finishedAt": r.finished_at,
        "steps": r.steps,
        "summary": r.summary,
        "error": r.error,
        "stats": r.context["stats"],
        "findings": r.context["findings"],
        "outputs": r.context["outputs"],
    }))
}

// ---- AI 对话自动编排：描述需求 → 自动挑选扩展并生成流程 ----

fn enabled_exts() -> Vec<Value> {
    load_store()["extensions"].as_array().cloned().unwrap_or_default()
        .into_iter().filter(|e| e["enabled"].as_bool().unwrap_or(false)).collect()
}

fn has(exts: &[Value], id: &str) -> bool { exts.iter().any(|e| e["id"].as_str() == Some(id)) }

/// 零成本规则编排：按关键词挑选已启用扩展，串行/并行流程
fn rule_compose(prompt: &str, exts: &[Value]) -> Value {
    let p = prompt.to_lowercase();
    let mut steps: Vec<Value> = vec![];
    fn push(steps: &mut Vec<Value>, exts: &[Value], key: &str, ext: &str, when: Option<Value>) {
        if !has(exts, ext) { return; }
        let mut st = json!({ "key": key, "ext": ext, "onError": "continue" });
        if let Some(w) = when { st["when"] = w; }
        steps.push(st);
    }
    let crawl = p.contains("全站") || p.contains("爬行") || p.contains("所有页面") || p.contains("crawl");
    if crawl && has(&exts, "agent-crawl") { push(&mut steps, &exts, "crawl", "agent-crawl", None); } else { push(&mut steps, &exts, "scan", "agent-rule", None); }
    if p.contains("秘钥") || p.contains("密钥") || p.contains("泄露") || p.contains("token") || p.contains("secret") {
        push(&mut steps, &exts, "secret", "skill-secret", None);
        push(&mut steps, &exts, "entropy", "skill-entropy", None);
    }
    if p.contains("响应头") || p.contains("header") || p.contains("cors") || p.contains("cookie") || p.contains("跨域") {
        push(&mut steps, &exts, "headers", "skill-header", None);
        push(&mut steps, &exts, "cors", "skill-cors", None);
        push(&mut steps, &exts, "cookie", "skill-cookie", None);
    }
    if p.contains("sql") || p.contains("注入") { push(&mut steps, &exts, "sqli", "skill-sqli", None); push(&mut steps, &exts, "sqli-pro", "skill-sqli-pro", None); }
    if p.contains("越权") || p.contains("idor") { push(&mut steps, &exts, "idor", "skill-idor", None); push(&mut steps, &exts, "idor-pro", "skill-idor-pro", None); }
    if p.contains("jwt") || p.contains("令牌") { push(&mut steps, &exts, "jwt", "skill-jwt", None); }
    if p.contains("ai") || p.contains("深度") || p.contains("逻辑") || p.contains("越权") || p.contains("资损") {
        push(&mut steps, &exts, "deep", "agent-ai", Some(json!({ "path": "aiEnabled", "op": "truthy" })));
    }
    push(&mut steps, &exts, "report", "bot-report", None);
    if steps.is_empty() { push(&mut steps, &exts, "scan", "agent-rule", None); push(&mut steps, &exts, "report", "bot-report", None); }
    let mode = if p.contains("并行") || p.contains("同时") { "parallel" } else { "serial" };
    json!({
        "name": format!("AI 编排 · {}", trunc(prompt, 24)),
        "description": "根据需求自动挑选扩展生成的协同流程",
        "mode": mode, "steps": steps,
    })
}

pub async fn ai_compose_pipeline(client: &reqwest::Client, prompt: &str) -> Result<Value, String> {
    let prompt = prompt.trim();
    if prompt.is_empty() { return Err("请描述你想要的巡检/挖掘功能".into()); }
    let exts = enabled_exts();
    if exts.is_empty() { return Err("请先在扩展中心启用至少一个扩展".into()); }
    let c = crate::ai::load_config();
    if c.enabled {
        let catalog: Vec<String> = exts.iter().map(|e| format!("- {}（{}）：{}", e["id"].as_str().unwrap_or(""), e["type"].as_str().unwrap_or(""), e["description"].as_str().unwrap_or(""))).collect();
        let system = format!(
            "你是 Web 安全协同编排助手。根据用户需求，从下列已启用扩展中挑选合适的扩展，生成一个编排 JSON（只输出 JSON 对象，不要其它文字）。\n可用扩展：\n{}\n输出字段：name, mode(serial/parallel/conditional), description, steps:[{{key, ext, onError(continue/stop), when?:{{path, op, value}}}}]。\n条件 op 可选：truthy/falsy/eq/ne/gt/gte/lt/lte/includes;常用 path：aiEnabled、stats.total、stats.critical。ext 必须是上面列出的 id。流程通常以扫描开始、以 bot-report 汇总结束。",
            catalog.join("\n"));
        if let Ok(r) = crate::ai::chat(client, json!([
            { "role": "system", "content": system },
            { "role": "user", "content": trunc(prompt, 2000) },
        ]), crate::ai::ChatOpts { config: Some(c.clone()), max_tokens: 1500, ..Default::default() }).await {
            let cleaned = r.content.replace("```json", "").replace("```", "");
            if let (Some(st), Some(en)) = (cleaned.find('{'), cleaned.rfind('}')) {
                if en > st {
                    if let Ok(v) = serde_json::from_str::<Value>(&cleaned[st..=en]) {
                        if let Ok(mut p) = validate_pipeline(&v) {
                            let valid: Vec<Value> = p["steps"].as_array().cloned().unwrap_or_default()
                                .into_iter().filter(|s| has(&exts, s["ext"].as_str().unwrap_or(""))).collect();
                            if !valid.is_empty() {
                                p["steps"] = json!(valid);
                                return Ok(json!({ "pipeline": p, "source": "ai", "model": r.model, "explanation": "AI 已自动挑选扩展并生成编排，可直接保存运行。" }));
                            }
                        }
                    }
                }
            }
        }
    }
    let p = validate_pipeline(&rule_compose(prompt, &exts))?;
    let note = if c.enabled { "AI 输出不可用，已按规则自动编排。" } else { "零成本模式：已按关键词自动挑选扩展并编排（启用 AI 可获得更智能的编排）。" };
    Ok(json!({ "pipeline": p, "source": "rule", "explanation": note }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_bad_mode_and_empty_steps() {
        assert!(validate_pipeline(&json!({ "mode": "nope", "steps": [{ "ext": "a" }] })).is_err());
        assert!(validate_pipeline(&json!({ "steps": [] })).is_err());
        assert!(validate_pipeline(&json!({ "steps": [{ "key": "k" }] })).is_err());
    }

    #[test]
    fn validate_clamps_and_defaults() {
        let p = validate_pipeline(&json!({
            "steps": [{ "ext": "agent-rule", "retry": 99, "timeoutMs": 99999999, "onError": "bogus" }],
        })).unwrap();
        assert_eq!(p["mode"], "serial");
        assert_eq!(p["steps"][0]["retry"], 3);
        assert_eq!(p["steps"][0]["timeoutMs"], 600000);
        assert_eq!(p["steps"][0]["onError"], "continue");
        assert_eq!(p["steps"][0]["key"], "agent-rule-1");
    }

    #[test]
    fn cond_js_semantics() {
        let ctx = json!({ "stats": { "total": 3, "high": 0 }, "url": "https://a.b", "list": ["x"], "flag": false });
        assert!(eval_cond(&json!({ "path": "stats.total", "op": "gt", "value": 2 }), &ctx));
        assert!(!eval_cond(&json!({ "path": "stats.high", "op": "truthy" }), &ctx));
        assert!(eval_cond(&json!({ "path": "stats.high", "op": "falsy" }), &ctx));
        assert!(eval_cond(&json!({ "path": "url", "op": "includes", "value": "a.b" }), &ctx));
        assert!(eval_cond(&json!({ "path": "list", "op": "includes", "value": "x" }), &ctx));
        assert!(!eval_cond(&json!({ "path": "flag", "op": "truthy" }), &ctx));
        assert!(!eval_cond(&json!({ "path": "missing", "op": "eq", "value": null }), &ctx));
        assert!(eval_cond(&Value::Null, &ctx));
    }

    #[test]
    fn merge_dedupes_and_counts() {
        let mut ctx = new_context("https://x");
        merge_findings(&mut ctx, &[
            json!({ "ruleId": "a", "title": "T1", "severity": "high" }),
            json!({ "ruleId": "a", "title": "T1", "severity": "high" }),
            json!({ "title": "T2", "severity": "low" }),
            json!({ "severity": "low" }),
        ]);
        assert_eq!(ctx["stats"]["total"], 2);
        assert_eq!(ctx["stats"]["high"], 1);
        assert_eq!(ctx["stats"]["low"], 1);
    }

    #[test]
    fn rule_compose_picks_enabled_exts() {
        let exts = vec![
            json!({ "id": "agent-rule", "enabled": true }),
            json!({ "id": "skill-secret", "enabled": true }),
            json!({ "id": "skill-entropy", "enabled": true }),
            json!({ "id": "bot-report", "enabled": true }),
            json!({ "id": "agent-ai", "enabled": false }),
        ];
        let p = rule_compose("扫描全站秘钥泄露", &exts);
        let ids: Vec<&str> = p["steps"].as_array().unwrap().iter().map(|s| s["ext"].as_str().unwrap()).collect();
        assert!(ids.contains(&"agent-crawl") == false); // agent-crawl 未启用
        assert!(ids.contains(&"agent-rule"));
        assert!(ids.contains(&"skill-secret"));
        assert!(ids.contains(&"bot-report"));
        assert!(!ids.contains(&"agent-ai")); // 禁用扩展不入选
        assert!(validate_pipeline(&p).is_ok());
    }
}