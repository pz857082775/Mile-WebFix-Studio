// AI 大模型接入：OpenAI 兼容协议；密钥仅存本机 ~/.webfix-studio/ai-config.json
// 零成本模式服务端强制：analyze 未启用时直接拒绝，绝不发起网络请求
use crate::util::{config_dir, join_url, trunc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "d_provider")]
    pub provider: String,
    #[serde(default, rename = "baseUrl")]
    pub base_url: String,
    #[serde(default, rename = "apiKey")]
    pub api_key: String,
    #[serde(default)]
    pub model: String,
}

fn d_provider() -> String { "deepseek".into() }

impl Default for AiConfig {
    fn default() -> Self {
        AiConfig { enabled: false, provider: d_provider(), base_url: String::new(), api_key: String::new(), model: String::new() }
    }
}

pub fn providers() -> Value {
    json!([
        { "id": "deepseek", "name": "DeepSeek", "baseUrl": "https://api.deepseek.com/v1", "models": ["deepseek-chat", "deepseek-reasoner"] },
        { "id": "qwen", "name": "通义千问（DashScope）", "baseUrl": "https://dashscope.aliyuncs.com/compatible-mode/v1", "models": ["qwen-plus", "qwen-max", "qwen-turbo"] },
        { "id": "kimi", "name": "Kimi（Moonshot）", "baseUrl": "https://api.moonshot.cn/v1", "models": ["moonshot-v1-8k", "moonshot-v1-32k", "moonshot-v1-128k"] },
        { "id": "glm", "name": "智谱 GLM", "baseUrl": "https://open.bigmodel.cn/api/paas/v4", "models": ["glm-4-plus", "glm-4-flash", "glm-4-air"] },
        { "id": "siliconflow", "name": "SiliconFlow", "baseUrl": "https://api.siliconflow.cn/v1", "models": ["deepseek-ai/DeepSeek-V3", "Qwen/Qwen2.5-72B-Instruct"] },
        { "id": "openai", "name": "OpenAI", "baseUrl": "https://api.openai.com/v1", "models": ["gpt-4o", "gpt-4o-mini"] },
        { "id": "ollama", "name": "Ollama 本地（零成本）", "baseUrl": "http://127.0.0.1:11434/v1", "models": ["qwen2.5:7b", "llama3.1:8b"] },
        { "id": "custom", "name": "自定义（OpenAI 兼容）", "baseUrl": "", "models": [] }
    ])
}

fn config_file() -> std::path::PathBuf { config_dir().join("ai-config.json") }

pub fn load_config() -> AiConfig {
    std::fs::read_to_string(config_file())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_config(patch: impl FnOnce(&mut AiConfig)) -> AiConfig {
    let mut c = load_config();
    patch(&mut c);
    let _ = std::fs::create_dir_all(config_dir());
    let _ = std::fs::write(config_file(), serde_json::to_string_pretty(&c).unwrap_or_default());
    c
}

pub fn public_config() -> Value {
    let c = load_config();
    let tail: String = c.api_key.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
    json!({
        "enabled": c.enabled,
        "provider": c.provider,
        "baseUrl": c.base_url,
        "model": c.model,
        "hasKey": !c.api_key.is_empty(),
        "keyTail": tail,
    })
}

pub struct ChatOpts {
    pub timeout_ms: u64,
    pub max_tokens: u64,
    pub config: Option<AiConfig>,
}

impl Default for ChatOpts {
    fn default() -> Self { ChatOpts { timeout_ms: 120_000, max_tokens: 2200, config: None } }
}

pub struct ChatResult {
    pub content: String,
    pub model: String,
    pub usage: Value,
}

pub async fn chat(client: &reqwest::Client, messages: Value, opts: ChatOpts) -> Result<ChatResult, String> {
    let c = opts.config.unwrap_or_else(load_config);
    if c.base_url.is_empty() {
        return Err("请先在「AI 设置」填写 API 地址".into());
    }
    if c.model.is_empty() {
        return Err("请先在「AI 设置」选择或填写模型名称".into());
    }
    let mut req = client.post(join_url(&c.base_url, "/chat/completions"))
        .timeout(Duration::from_millis(opts.timeout_ms))
        .header("Content-Type", "application/json");
    if !c.api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", c.api_key));
    }
    let r = req.json(&json!({
        "model": c.model,
        "messages": messages,
        "temperature": 0.2,
        "max_tokens": opts.max_tokens,
    })).send().await.map_err(|e| {
        if e.is_timeout() { "模型响应超时，请稍后重试或换更快的模型".to_string() }
        else if e.is_connect() { "无法连接 API 地址（本地 Ollama 请确认已启动）".to_string() }
        else { format!("请求失败：{}", e) }
    })?;

    let status = r.status();
    let text = r.text().await.unwrap_or_default();
    if !status.is_success() {
        let msg = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| v["error"]["message"].as_str().map(String::from))
            .unwrap_or_else(|| trunc(&text, 300));
        return Err(format!("模型服务返回 {}：{}", status.as_u16(), msg));
    }
    let data: Value = serde_json::from_str(&text).map_err(|_| "模型响应不是合法 JSON".to_string())?;
    let content = data["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string();
    if content.is_empty() {
        return Err("模型未返回内容".into());
    }
    Ok(ChatResult {
        content,
        model: data["model"].as_str().unwrap_or(&c.model).to_string(),
        usage: if data["usage"].is_null() { Value::Null } else { data["usage"].clone() },
    })
}

pub async fn list_models(client: &reqwest::Client, c: &AiConfig) -> Result<Vec<String>, String> {
    if c.base_url.is_empty() {
        return Err("请先填写 API 地址".into());
    }
    let mut req = client.get(join_url(&c.base_url, "/models")).timeout(Duration::from_secs(15));
    if !c.api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", c.api_key));
    }
    let r = req.send().await.map_err(|e| format!("无法连接 API 地址：{}", e))?;
    if !r.status().is_success() {
        return Err(format!("模型列表返回 {}（部分服务商不支持 /models，可直接手填模型名）", r.status().as_u16()));
    }
    let data: Value = r.json().await.map_err(|_| "模型列表响应不是合法 JSON".to_string())?;
    // 兼容多种返回结构：OpenAI {data:[{id}]}、{models:[{id|name}]}、纯数组 [..]
    let arr = data["data"].as_array()
        .or_else(|| data["models"].as_array())
        .or_else(|| data.as_array());
    Ok(arr.map(|a| {
        a.iter().filter_map(|m| {
            m["id"].as_str().or_else(|| m["name"].as_str()).or_else(|| m.as_str())
                .map(String::from)
        }).take(300).collect()
    }).unwrap_or_default())
}

pub async fn test_connection(client: &reqwest::Client, c: AiConfig) -> Result<Value, String> {
    let r = chat(client, json!([{ "role": "user", "content": "连通性测试，请只回复：OK" }]),
        ChatOpts { timeout_ms: 30_000, max_tokens: 10, config: Some(c) }).await?;
    Ok(json!({ "ok": true, "model": r.model, "reply": trunc(&r.content, 50) }))
}

const SYSTEM: &str = "你是资深 Web 安全与质量审计专家。基于用户给出的扫描证据，挖掘内置规则难以发现的 BUG 与漏洞：业务逻辑、越权、资损、数据一致性、算法边界、性能、无障碍等。\n要求：\n- 只输出 JSON 数组，最多 8 条，不要输出 JSON 以外的任何文字（包括代码块标记）。\n- 每条字段：title(中文一句话), category(security/ui/function/logic/data/algorithm/performance/a11y), severity(critical/high/medium/low), rootCause(一句话根因), test(一句话挖掘/复现方法), fix:{frontend,backend}(最简修复代码或指引，无需改动的写\"无需改动\"), verify(1-2 条验证步骤数组), guard(一句防同类复发的防护)。\n- 不要重复「已发现」清单里的问题；证据不足、没有把握的不要输出；修复建议必须最小改动。";

const CATS: [&str; 8] = ["security", "ui", "function", "logic", "data", "algorithm", "performance", "a11y"];
const SEVS: [&str; 4] = ["critical", "high", "medium", "low"];

pub async fn analyze(client: &reqwest::Client, ev: &Value) -> Result<Value, String> {
    let c = load_config();
    if !c.enabled {
        return Err("请先在「AI 设置」启用 AI 增强模式".into());
    }
    let known = ev["findings"].as_array().map(|a| {
        a.iter().map(|f| format!("- {}（{}）", f["title"].as_str().unwrap_or(""), f["severity"].as_str().unwrap_or(""))).collect::<Vec<_>>().join("\n")
    }).filter(|s| !s.is_empty()).unwrap_or_else(|| "（无）".into());

    let console_lines = ev["console"].as_array().map(|a| {
        a.iter().take(20).map(|e| {
            let url = e["realUrl"].as_str().filter(|s| !s.is_empty())
                .or_else(|| e["url"].as_str()).unwrap_or("");
            format!("[{}] {} {}", e["type"].as_str().unwrap_or(""), e["message"].as_str().unwrap_or(""), url)
        }).collect::<Vec<_>>().join("\n")
    }).unwrap_or_default();

    let headers_str = trunc(&serde_json::to_string_pretty(&ev["headers"]).unwrap_or_else(|_| "{}".into()), 2500);
    let mut parts: Vec<String> = vec![
        format!("目标页面：{}（证据类型：{}）",
            ev["url"].as_str().filter(|s| !s.is_empty()).unwrap_or("（未提供）"),
            ev["kind"].as_str().filter(|s| !s.is_empty()).unwrap_or("scan")),
    ];
    if let Some(pages) = ev["pages"].as_array() {
        if !pages.is_empty() {
            parts.push(format!("爬行覆盖页面：\n{}", pages.iter().take(30).filter_map(|p| p.as_str()).collect::<Vec<_>>().join("\n")));
        }
    }
    parts.push(format!("响应头：\n{}", headers_str));
    if !console_lines.is_empty() {
        parts.push(format!("浏览器控制台/异常/网络抓取：\n{}", console_lines));
    }
    parts.push(format!("已发现（不要重复）：\n{}", known));
    if let Some(html) = ev["html"].as_str() {
        if !html.is_empty() {
            parts.push(format!("页面 HTML 片段：\n{}", trunc(html, 12_000)));
        }
    }
    parts.push("请输出 JSON 数组。".into());
    let user = parts.join("\n\n");

    let r = chat(client, json!([
        { "role": "system", "content": SYSTEM },
        { "role": "user", "content": user },
    ]), ChatOpts::default()).await?;

    let cleaned = r.content.replace("```json", "").replace("```", "");
    let start = cleaned.find('[');
    let end = cleaned.rfind(']');
    let arr: Vec<Value> = match (start, end) {
        (Some(s), Some(e)) if e > s => serde_json::from_str(&cleaned[s..=e]).map_err(|_| "模型输出无法解析为 JSON，请重试或换模型".to_string())?,
        _ => return Err("模型输出无法解析为 JSON，请重试或换模型".into()),
    };

    let ev_url = ev["url"].as_str().unwrap_or("");
    let findings: Vec<Value> = arr.iter().take(8).enumerate().map(|(i, it)| {
        let category = it["category"].as_str().filter(|c| CATS.contains(c)).unwrap_or("function");
        let severity = it["severity"].as_str().filter(|s| SEVS.contains(s)).unwrap_or("medium");
        let root = trunc(it["rootCause"].as_str().unwrap_or(""), 400);
        let test_or_root = it["test"].as_str().filter(|s| !s.is_empty()).unwrap_or_else(|| it["rootCause"].as_str().unwrap_or(""));
        let mut f = json!({
            "ruleId": Value::Null,
            "source": "ai",
            "id": format!("ai-{}", i),
            "title": trunc(it["title"].as_str().filter(|s| !s.is_empty()).unwrap_or("AI 发现"), 120),
            "category": category,
            "severity": severity,
            "rootCause": root,
            "evidence": format!("AI 深度挖掘：{}", trunc(test_or_root, 300)),
            "url": ev_url,
        });
        if it["fix"].is_object() {
            f["fix"] = json!({
                "frontend": it["fix"]["frontend"].as_str().unwrap_or(""),
                "backend": it["fix"]["backend"].as_str().unwrap_or(""),
            });
        }
        if let Some(v) = it["verify"].as_array() {
            f["verify"] = json!(v.iter().take(3).map(|x| match x {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            }).collect::<Vec<_>>());
        }
        if let Some(g) = it["guard"].as_str() {
            if !g.is_empty() {
                f["guard"] = json!(trunc(g, 400));
            }
        }
        f
    }).filter(|f| !f["title"].as_str().unwrap_or("").is_empty()).collect();

    Ok(json!({ "findings": findings, "model": r.model, "usage": r.usage }))
}