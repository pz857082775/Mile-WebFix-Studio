// 全站爬行：BFS + sitemap 发现，跨页聚合去重（与 JS crawler.js 对齐）
use crate::kb::summarize;
use crate::scanner::{scan_target, ScanOpts};
use crate::util::now_ms;
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use url::Url;

pub struct CrawlJob {
    pub id: String,
    pub seed: String,
    pub status: String,
    pub started_at: u64,
    pub finished_at: Option<u64>,
    pub pages: Vec<Value>,
    pub scanned: u64,
    pub max_pages: u64,
    pub max_depth: u64,
    pub findings: Vec<Value>,
    pub finding_index: HashMap<String, usize>,
    pub error: Option<String>,
}

pub type SharedJob = Arc<Mutex<CrawlJob>>;

pub fn job_json(job: &CrawlJob) -> Value {
    json!({
        "id": job.id,
        "seed": job.seed,
        "status": job.status,
        "error": job.error,
        "scanned": job.scanned,
        "maxPages": job.max_pages,
        "pages": job.pages,
        "findings": job.findings,
        "summary": summarize(&job.findings),
        "durationMs": job.finished_at.unwrap_or_else(now_ms) - job.started_at,
    })
}

pub fn start_crawl(
    follow: Client,
    manual: Client,
    url: &str,
    max_pages: Option<u64>,
    max_depth: Option<u64>,
) -> SharedJob {
    let id = uuid::Uuid::new_v4().to_string();
    let job = Arc::new(Mutex::new(CrawlJob {
        id: id.clone(),
        seed: url.to_string(),
        status: "running".into(),
        started_at: now_ms(),
        finished_at: None,
        pages: Vec::new(),
        scanned: 0,
        max_pages: max_pages.unwrap_or(30).clamp(1, 100),
        max_depth: max_depth.unwrap_or(3).clamp(1, 6),
        findings: Vec::new(),
        finding_index: HashMap::new(),
        error: None,
    }));
    let j = job.clone();
    tokio::spawn(async move {
        let result = run(&follow, &manual, &j).await;
        let mut job = j.lock().await;
        match result {
            Ok(()) => job.status = "done".into(),
            Err(e) => {
                job.status = "error".into();
                job.error = Some(e);
            }
        }
        job.finished_at = Some(now_ms());
    });
    job
}

async fn run(follow: &Client, manual: &Client, job: &SharedJob) -> Result<(), String> {
    let (seed, max_pages, max_depth, started_at) = {
        let j = job.lock().await;
        (j.seed.clone(), j.max_pages, j.max_depth, j.started_at)
    };
    let seed_url = Url::parse(&seed).map_err(|_| "URL 格式不正确".to_string())?;
    let mut queue: VecDeque<(String, u64)> = VecDeque::new();
    let mut seen: HashSet<String> = HashSet::new();
    queue.push_back((seed_url.to_string(), 0));
    seen.insert(seed_url.to_string());

    // sitemap.xml 补充发现路由
    let origin = seed_url.origin().ascii_serialization();
    if let Ok(r) = follow.get(format!("{}/sitemap.xml", origin))
        .timeout(Duration::from_secs(5)).send().await
    {
        if r.status().is_success() {
            let xml = r.text().await.unwrap_or_default();
            let re = regex::Regex::new(r"(?i)<loc>\s*([^<\s]+)\s*</loc>").unwrap();
            for c in re.captures_iter(&xml) {
                if let Ok(mut u) = Url::parse(&c[1]) {
                    u.set_fragment(None);
                    if u.origin().ascii_serialization() == origin && !seen.contains(u.as_str()) {
                        seen.insert(u.to_string());
                        queue.push_back((u.to_string(), 0));
                    }
                }
            }
        }
    }

    loop {
        let (scanned, elapsed_ok) = {
            let j = job.lock().await;
            (j.scanned, now_ms() - started_at < 180_000)
        };
        if queue.is_empty() || scanned >= max_pages || !elapsed_ok {
            break;
        }
        let Some((url, depth)) = queue.pop_front() else { break };
        match scan_target(follow, manual, &url, ScanOpts { probe_paths: false, discover: depth == 0, active: true, assets: true }).await {
            Ok(result) => {
                let findings = result["findings"].as_array().cloned().unwrap_or_default();
                let links = result["links"].as_array().cloned().unwrap_or_default();
                let mut j = job.lock().await;
                j.pages.push(json!({
                    "url": url,
                    "status": result["status"],
                    "count": findings.len(),
                    "titles": findings.iter().map(|f| f["title"].clone()).collect::<Vec<_>>(),
                    "findings": findings.iter().map(|f| json!({
                        "ruleId": f["ruleId"],
                        "title": f["title"],
                        "category": f["category"],
                        "severity": f["severity"],
                        "evidence": f["evidence"],
                    })).collect::<Vec<_>>(),
                }));
                for f in &findings {
                    let key = format!("{}|{}", f["ruleId"].as_str().unwrap_or(""), f["evidence"].as_str().unwrap_or(""));
                    let idx = match j.finding_index.get(&key) {
                        Some(&i) => i,
                        None => {
                            let mut agg = f.clone();
                            agg["urls"] = json!([]);
                            j.findings.push(agg);
                            let i = j.findings.len() - 1;
                            j.finding_index.insert(key, i);
                            i
                        }
                    };
                    let urls = j.findings[idx]["urls"].as_array_mut().unwrap();
                    if !urls.iter().any(|u| u.as_str() == Some(url.as_str())) {
                        urls.push(json!(url));
                    }
                }
                if depth < max_depth {
                    for link in links {
                        let Some(l) = link.as_str() else { continue };
                        if !seen.contains(l) && (seen.len() as u64) < max_pages * 4 {
                            seen.insert(l.to_string());
                            queue.push_back((l.to_string(), depth + 1));
                        }
                    }
                }
            }
            Err(e) => {
                let mut j = job.lock().await;
                j.pages.push(json!({ "url": url, "error": e, "count": 0, "titles": [] }));
            }
        }
        {
            let mut j = job.lock().await;
            j.scanned += 1;
        }
        tokio::time::sleep(Duration::from_millis(120)).await;
    }
    Ok(())
}