// 知识库：内嵌 assets/knowledge.json（由旧版 JS 引擎精确导出，261 条规则逐条一致）
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Deserialize)]
pub struct Fix {
    pub frontend: String,
    pub backend: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    pub category: String,
    pub severity: String,
    pub title: String,
    #[serde(rename = "rootCause")]
    pub root_cause: String,
    #[serde(default)]
    pub test: String,
    pub fix: Fix,
    pub verify: Vec<String>,
    pub guard: String,
    #[serde(default)]
    pub diff: String,
}

#[derive(Deserialize)]
struct KbFile {
    rules: HashMap<String, Rule>,
    manual: HashMap<String, Rule>,
    #[serde(rename = "captureRules")]
    capture_rules: HashMap<String, Rule>,
    knowledge: HashMap<String, Rule>,
}

pub struct Kb {
    /// 合并后的全量规则（RULES + MANUAL + CAPTURE_RULES + KNOWLEDGE，后者覆盖前者，与 JS allRules 一致）
    pub all: HashMap<String, Rule>,
    /// 挖掘模板（MANUAL + KNOWLEDGE）
    pub manual_ids: HashMap<String, bool>,
    /// 保持导出顺序的 id 列表（rules → manual → capture → knowledge）
    pub order: Vec<String>,
}

static KB: OnceLock<Kb> = OnceLock::new();

pub fn kb() -> &'static Kb {
    KB.get_or_init(|| {
        let raw = include_str!("../../../assets/knowledge.json");
        let file: KbFile = serde_json::from_str(raw).expect("knowledge.json 解析失败");
        let mut all = HashMap::new();
        let mut order = Vec::new();
        let mut manual_ids = HashMap::new();
        for dict in [&file.rules, &file.manual, &file.capture_rules, &file.knowledge] {
            for (id, r) in dict {
                if !all.contains_key(id) {
                    order.push(id.clone());
                }
                all.insert(id.clone(), r.clone());
            }
        }
        for id in file.manual.keys().chain(file.knowledge.keys()) {
            manual_ids.insert(id.clone(), true);
        }
        Kb { all, manual_ids, order }
    })
}

pub fn get_rule(id: &str) -> Option<&'static Rule> {
    kb().all.get(id)
}

pub fn is_manual(id: &str) -> bool {
    kb().manual_ids.contains_key(id)
}

/// 与 JS enrichFindings 一致：补 title/category/severity
pub fn enrich_findings(raw: &[Value]) -> Vec<Value> {
    raw.iter()
        .map(|f| {
            let rule_id = f.get("ruleId").and_then(|v| v.as_str()).unwrap_or("");
            let r = get_rule(rule_id);
            let mut m = f.as_object().cloned().unwrap_or_default();
            m.insert("title".into(), json!(r.map(|x| x.title.clone()).unwrap_or_else(|| rule_id.to_string())));
            m.insert("category".into(), json!(r.map(|x| x.category.clone()).unwrap_or_else(|| "function".into())));
            m.insert("severity".into(), json!(r.map(|x| x.severity.clone()).unwrap_or_else(|| "medium".into())));
            Value::Object(m)
        })
        .collect()
}

pub fn summarize(findings: &[Value]) -> Value {
    let mut by_severity = Map::new();
    let mut by_category = Map::new();
    for f in findings {
        let sev = f.get("severity").and_then(|v| v.as_str()).unwrap_or("medium");
        let cat = f.get("category").and_then(|v| v.as_str()).unwrap_or("function");
        let s = by_severity.get(sev).and_then(|v| v.as_u64()).unwrap_or(0);
        by_severity.insert(sev.into(), json!(s + 1));
        let c = by_category.get(cat).and_then(|v| v.as_u64()).unwrap_or(0);
        by_category.insert(cat.into(), json!(c + 1));
    }
    json!({ "total": findings.len(), "bySeverity": by_severity, "byCategory": by_category })
}

/// /api/rules 输出（与 JS 字段顺序与内容一致）
pub fn rules_json() -> Vec<Value> {
    kb().order
        .iter()
        .filter_map(|id| {
            let r = kb().all.get(id)?;
            Some(json!({
                "id": id,
                "manual": is_manual(id),
                "category": r.category,
                "severity": r.severity,
                "title": r.title,
                "rootCause": r.root_cause,
                "test": r.test,
                "fix": { "frontend": r.fix.frontend, "backend": r.fix.backend },
                "verify": r.verify,
                "guard": r.guard,
                "diff": r.diff,
            }))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kb_counts_match_legacy() {
        assert_eq!(kb().all.len(), 261);
        assert!(get_rule("sec-missing-csp").is_some());
        assert!(get_rule("sec-sqli").is_some());
        assert!(is_manual("sec-sqli"));
        assert!(!is_manual("sec-missing-csp"));
    }

    #[test]
    fn enrich_defaults() {
        let raw = vec![json!({"ruleId": "sec-missing-csp", "evidence": "x"})];
        let out = enrich_findings(&raw);
        assert_eq!(out[0]["severity"], "medium");
        assert_eq!(out[0]["category"], "security");
    }
}