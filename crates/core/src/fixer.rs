// 修复四件套 + 精简/详细提示词 + 补丁 + 报告（与 JS fixer.js 逐字对齐）
use crate::kb::{get_rule, Rule};
use crate::util::trunc;
use serde_json::{json, Value};

fn sev_cn(s: &str) -> &str {
    match s { "critical" => "严重", "high" => "高", "medium" => "中", "low" => "低", _ => s }
}

fn cat_cn(s: &str) -> &str {
    match s {
        "security" => "安全漏洞", "ui" => "界面", "function" => "功能", "logic" => "逻辑",
        "data" => "数据", "algorithm" => "算法", "performance" => "性能", "a11y" => "无障碍",
        _ => s,
    }
}

fn fix_hint(rule: Option<&Rule>) -> String {
    let Some(rule) = rule else { return "按根因定位后最小改动修复".into() };
    let parts: Vec<&str> = [&rule.fix.backend, &rule.fix.frontend].into_iter()
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty() && !s.starts_with("// 无需"))
        .collect();
    let text = parts.join("\n");
    let lines: Vec<&str> = text.split('\n').map(|s| s.trim()).collect();
    let after_idx = lines.iter().position(|s| s.contains("改后") || s.contains("修复后"));
    let pool: &[&str] = match after_idx {
        Some(i) => &lines[i + 1..],
        None => &lines,
    };
    let line = pool.iter().find(|s| {
        !s.is_empty() && !s.starts_with("//") && !s.starts_with('#') && !s.starts_with("<!--") && !s.starts_with('-')
    });
    let chosen = line.copied().or_else(|| lines.first().copied()).unwrap_or("见修复方案");
    trunc(chosen, 140)
}

fn s(v: &Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

/// 精简提示词：问题描述 + 如何修复，不啰嗦
pub fn build_brief_prompt(f: &Value, rule: Option<&Rule>) -> String {
    let title = s(f, "title");
    let severity = s(f, "severity");
    let category = s(f, "category");
    let url = s(f, "url");
    let root = {
        let r = s(f, "rootCause");
        if !r.is_empty() { r } else { rule.map(|x| x.root_cause.clone()).unwrap_or_default() }
    };
    let evidence = s(f, "evidence");
    let mut lines = vec![
        format!("【问题】{}（{} · {}）", title, sev_cn(&severity), cat_cn(&category)),
        format!("【页面】{}", if url.is_empty() { "（未提供）".to_string() } else { url }),
        format!("【描述】{}", trunc(&root, 200)),
    ];
    if !evidence.is_empty() {
        lines.push(format!("【证据】{}", trunc(&evidence, 200)));
    }
    lines.push(format!("【修复】{}", fix_hint(rule)));
    lines.push("直接输出：最简修复代码（前端/后端分开）+ 可提交的 git diff。不要解释概念。".into());
    lines.join("\n")
}

pub fn build_prompt(f: &Value, stack: &Value) -> String {
    let frontend = stack.get("frontend").and_then(|v| v.as_str()).unwrap_or("原生 HTML/JS");
    let backend = stack.get("backend").and_then(|v| v.as_str()).unwrap_or("Node.js (Express)");
    let url = s(f, "url");
    let evidence = s(f, "evidence");
    [
        "你是资深全栈开发。请针对以下网页 BUG/漏洞给出修复：".to_string(),
        String::new(),
        format!("【BUG标题】：{}", s(f, "title")),
        format!("【页面URL】：{}", if url.is_empty() { "（未提供）".to_string() } else { url }),
        format!("【复现步骤】：{}", s(f, "steps")),
        format!("【截图/证据】：{}", if evidence.is_empty() { "（无）".to_string() } else { evidence }),
        format!("【风险等级】：{}", s(f, "severity")),
        format!("【根因分析】：{}", s(f, "rootCause")),
        format!("【技术栈】：前端 {}；后端 {}", frontend, backend),
        String::new(),
        "请输出：".into(),
        "1. 最简修复方案代码（区分前端/后端）".into(),
        "2. 修复后验证步骤".into(),
        "3. 补充防止同类BUG的防护代码".into(),
        "4. 把修复代码做成可直接提交的git diff".into(),
        String::new(),
        "要求：最小改动；diff 基于真实文件路径，不确定处标注假设；防护代码可直接用于 CI。".into(),
    ].join("\n")
}

pub fn build_deliverables(input: &Value, stack: &Value) -> Value {
    let rule_id = s(input, "ruleId");
    let rule = if rule_id.is_empty() { None } else { get_rule(&rule_id) };

    let pick = |key: &str, fallback: &str| -> String {
        let v = s(input, key);
        if !v.is_empty() { return v; }
        let r = match key {
            "title" => rule.map(|x| x.title.clone()),
            "severity" => rule.map(|x| x.severity.clone()),
            "rootCause" => rule.map(|x| x.root_cause.clone()),
            "category" => rule.map(|x| x.category.clone()),
            _ => None,
        };
        r.unwrap_or_else(|| fallback.to_string())
    };

    let steps = {
        let v = s(input, "steps");
        if v.is_empty() { "运行一键体检后由扫描器自动发现，详见证据字段".to_string() } else { v }
    };
    let fields = json!({
        "title": pick("title", "未命名问题"),
        "url": s(input, "url"),
        "steps": steps,
        "evidence": s(input, "evidence"),
        "severity": pick("severity", "medium"),
        "rootCause": pick("rootCause", "待结合源码进一步分析"),
        "category": pick("category", "security"),
    });

    let minimal_fix = if input.get("fix").map(|v| v.is_object()).unwrap_or(false) {
        json!({
            "frontend": input["fix"].get("frontend").and_then(|v| v.as_str()).unwrap_or(""),
            "backend": input["fix"].get("backend").and_then(|v| v.as_str()).unwrap_or(""),
        })
    } else if let Some(r) = rule {
        json!({ "frontend": r.fix.frontend, "backend": r.fix.backend })
    } else {
        json!({
            "frontend": "// 请提供前端技术栈与相关代码片段，以生成针对性最简修复",
            "backend": "// 请提供后端技术栈与相关代码片段，以生成针对性最简修复",
        })
    };

    let verify_steps = if input.get("verify").map(|v| v.is_array()).unwrap_or(false) {
        input["verify"].clone()
    } else if let Some(r) = rule {
        json!(r.verify)
    } else {
        json!(["修复后重新运行一键体检，确认该发现消失", "手动回归相关功能，确认无副作用"])
    };

    let guard_code = {
        let v = s(input, "guard");
        if !v.is_empty() { v }
        else if let Some(r) = rule { r.guard.clone() }
        else { "// 在 CI 中加入对应 lint 规则与回归用例，防止同类问题复发".into() }
    };

    let git_diff = rule.map(|r| r.diff.clone()).filter(|d| !d.is_empty())
        .unwrap_or_else(|| "# 该问题暂无预置补丁，请基于实际项目文件生成 diff".into());

    let deliverables = json!({
        "minimalFix": minimal_fix,
        "verifySteps": verify_steps,
        "guardCode": guard_code,
        "gitDiff": git_diff,
    });

    let mut brief_fields = fields.clone();
    brief_fields["ruleId"] = json!(if rule_id.is_empty() { Value::Null } else { json!(rule_id) });
    let prompt = build_brief_prompt(&brief_fields, rule);
    let full_prompt = build_prompt(&fields, stack);

    json!({
        "ruleId": if rule_id.is_empty() { Value::Null } else { json!(rule_id) },
        "fields": fields,
        "deliverables": deliverables,
        "prompt": prompt,
        "fullPrompt": full_prompt,
    })
}

pub fn build_patch(items: &[Value]) -> String {
    let header = "# WebFix Studio 建议补丁\n# 注意：文件路径为典型示例路径，应用前请按实际项目调整。\n# 应用方式：保存为 fix.patch 后执行 git apply --check fix.patch 预检，再 git apply fix.patch\n";
    let body: Vec<String> = items.iter().enumerate().map(|(i, it)| {
        format!("# ---- {}. {}（{}）----\n{}\n",
            i + 1,
            it.get("title").and_then(|v| v.as_str()).unwrap_or(""),
            it.get("severity").and_then(|v| v.as_str()).unwrap_or(""),
            it.get("gitDiff").and_then(|v| v.as_str()).unwrap_or(""))
    }).collect();
    format!("{}\n{}", header, body.join("\n"))
}

pub fn build_report_md(result: &Value) -> String {
    let f = &result["fields"];
    let d = &result["deliverables"];
    let url = s(f, "url");
    let evidence = s(f, "evidence");
    let verify: Vec<String> = d["verifySteps"].as_array().map(|a| {
        a.iter().enumerate().map(|(i, v)| format!("{}. {}", i + 1, v.as_str().unwrap_or(""))).collect()
    }).unwrap_or_default();
    format!(
        "# BUG/漏洞修复报告\n\n- 【BUG标题】：{title}\n- 【页面URL】：{url}\n- 【复现步骤】：{steps}\n- 【截图/证据】：{evidence}\n- 【风险等级】：{severity}\n- 【根因分析】：{root}\n\n## 1. 最简修复方案代码\n\n### 前端\n\n```\n{fe}\n```\n\n### 后端\n\n```\n{be}\n```\n\n## 2. 修复后验证步骤\n\n{verify}\n\n## 3. 防止同类 BUG 的防护代码\n\n```\n{guard}\n```\n\n## 4. Git Diff（建议补丁，路径需按实际项目调整）\n\n```diff\n{diff}\n```\n",
        title = s(f, "title"),
        url = if url.is_empty() { "（未提供）".to_string() } else { url },
        steps = s(f, "steps"),
        evidence = if evidence.is_empty() { "（无）".to_string() } else { evidence },
        severity = s(f, "severity"),
        root = s(f, "rootCause"),
        fe = d["minimalFix"]["frontend"].as_str().unwrap_or(""),
        be = d["minimalFix"]["backend"].as_str().unwrap_or(""),
        verify = verify.join("\n"),
        guard = d["guardCode"].as_str().unwrap_or(""),
        diff = d["gitDiff"].as_str().unwrap_or(""),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deliverables_from_rule() {
        let input = json!({ "ruleId": "sec-missing-csp", "url": "https://x.com" });
        let out = build_deliverables(&input, &json!({}));
        assert_eq!(out["fields"]["severity"], "medium");
        assert!(out["deliverables"]["gitDiff"].as_str().unwrap().contains("nginx.conf"));
        assert!(out["prompt"].as_str().unwrap().contains("【问题】"));
        assert!(out["fullPrompt"].as_str().unwrap().contains("【BUG标题】"));
    }

    #[test]
    fn brief_prompt_concise() {
        let input = json!({ "ruleId": "sec-sqli", "title": "SQL 注入", "severity": "critical", "category": "security", "evidence": "e" });
        let out = build_deliverables(&input, &json!({}));
        let p = out["prompt"].as_str().unwrap();
        assert!(p.contains("【修复】"));
        assert!(p.lines().count() <= 7);
    }
}