// WebFix Studio 核心库：网页体检 / 全站爬行 / 巡检代理 / 修复四件套 / AI 增强 / 扩展协同编排
pub mod ai;
pub mod crawler;
pub mod extensions;
pub mod fixer;
pub mod kb;
pub mod orchestrator;
pub mod proxy;
pub mod scanner;
pub mod server;
pub mod util;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct Session {
    pub events: Vec<serde_json::Value>,
    pub created_at: u64,
    pub target: Option<String>,
}

pub struct AppState {
    /// 跟随重定向（体检 / 爬行）
    pub follow: reqwest::Client,
    /// 不跟随重定向（敏感路径探测 / 巡检代理）
    pub manual: reqwest::Client,
    pub sessions: Mutex<HashMap<String, Session>>,
    pub crawls: Mutex<HashMap<String, crawler::SharedJob>>,
    pub runs: Mutex<Vec<(String, orchestrator::SharedRun)>>,
}

impl AppState {
    pub fn new() -> Arc<Self> {
        let follow = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(20))
            .build()
            .expect("HTTP 客户端构建失败");
        let manual = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("HTTP 客户端构建失败");
        Arc::new(AppState {
            follow,
            manual,
            sessions: Mutex::new(HashMap::new()),
            crawls: Mutex::new(HashMap::new()),
            runs: Mutex::new(Vec::new()),
        })
    }

    pub async fn add_capture(&self, sid: &str, mut event: serde_json::Value) {
        if sid.is_empty() {
            return;
        }
        let mut sessions = self.sessions.lock().await;
        let s = sessions.entry(sid.to_string()).or_insert_with(|| Session {
            events: Vec::new(),
            created_at: util::now_ms(),
            target: None,
        });
        if event.get("ts").and_then(|v| v.as_u64()).unwrap_or(0) == 0 {
            event["ts"] = serde_json::json!(util::now_ms());
        }
        s.events.push(event);
        if s.events.len() > 1000 {
            let overflow = s.events.len() - 1000;
            s.events.drain(0..overflow);
        }
    }
}

/// 在候选端口中启动服务（8787–8807 回退），返回 (端口, 服务任务句柄)
pub async fn start_server(state: Arc<AppState>, ports: &[u16]) -> Result<(u16, tokio::task::JoinHandle<()>), String> {
    for &p in ports {
        if let Ok(listener) = tokio::net::TcpListener::bind(("127.0.0.1", p)).await {
            let app = server::router(state.clone());
            let handle = tokio::spawn(async move {
                let _ = axum::serve(listener, app).await;
            });
            return Ok((p, handle));
        }
    }
    Err("所有候选端口均被占用".into())
}

pub fn default_ports() -> Vec<u16> {
    (8787..=8807).collect()
}