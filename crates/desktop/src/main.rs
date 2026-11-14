// WebFix Studio 桌面壳：进程内启动核心服务，WebView 加载与网页版完全相同的前端
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::webview::NewWindowResponse;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use webfix_core::{default_ports, start_server, AppState};

fn open_external(url: &str) {
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
    let _ = cmd.spawn();
}

fn is_local(url: &tauri::Url) -> bool {
    matches!(url.host_str(), Some("127.0.0.1") | Some("localhost") | Some("[::1]") | Some("::1"))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .setup(|app| {
            // 进程内启动核心服务（8787–8807 回退），与网页版共用同一引擎与前端
            let state = AppState::new();
            let (port, _handle) = tauri::async_runtime::block_on(start_server(state, &default_ports()))
                .map_err(|e| format!("核心服务启动失败：{e}"))?;
            let url = format!("http://127.0.0.1:{port}");
            let parsed: tauri::Url = url.parse().map_err(|e| format!("URL 解析失败：{e}"))?;

            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(parsed))
                .title("WebFix Studio · 网页 BUG / 漏洞巡检修复工作台")
                .inner_size(1280.0, 820.0)
                .min_inner_size(980.0, 640.0)
                .center()
                .decorations(false)
                .on_navigation(|url| {
                    if is_local(url) {
                        true
                    } else {
                        // 外部链接交给系统浏览器，应用内只保留本地工作台
                        open_external(url.as_str());
                        false
                    }
                })
                .on_new_window(|url, _features| {
                    // target=_blank / window.open（如 /demo 演示靶场）交给系统浏览器
                    open_external(url.as_str());
                    NewWindowResponse::Deny
                })
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("WebFix Studio 桌面版运行失败");
}