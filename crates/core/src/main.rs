// WebFix Studio 服务端入口：网页版 / 桌面版共用同一核心
use webfix_core::{default_ports, start_server, AppState};

#[tokio::main]
async fn main() {
    let state = AppState::new();
    let ports: Vec<u16> = match std::env::var("PORT").ok().and_then(|p| p.parse::<u16>().ok()) {
        Some(p) => vec![p],
        None => default_ports(),
    };
    let (port, _handle) = match start_server(state, &ports).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("启动失败：{}", e);
            std::process::exit(1);
        }
    };
    let url = format!("http://127.0.0.1:{}", port);
    println!("WebFix Studio 已启动：{}", url);
    println!("演示靶场：{}/demo（含登录墙 {}/demo/admin）", url, url);
    if std::env::args().any(|a| a == "--open") {
        let _ = open_browser(&url);
    }
    // 服务任务在后台运行，主任务保持存活
    futures::future::pending::<()>().await;
}

fn open_browser(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", "", url]);
        c
    };
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = std::process::Command::new("open");
        c.arg(url);
        c
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(url);
        c
    };
    cmd.spawn().map(|_| ())
}