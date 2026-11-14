// WebFix Studio 桌面壳：进程内启动核心服务，WebView 加载与网页版完全相同的前端
// 托盘：右键菜单（打开软件 / 设置软件 / 退出），左键唤起，关闭按钮最小化到托盘
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::webview::NewWindowResponse;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};
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

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "打开软件", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "设置软件", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &sep, &settings, &quit])?;

    let mut tray = TrayIconBuilder::with_id("main-tray")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("WebFix Studio · 网页 BUG / 漏洞巡检修复工作台")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "settings" => {
                show_main(app);
                let _ = app.emit("wf-navigate", "ai");
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            show_main(app);
        }))
        .setup(|app| {
            // 进程内启动核心服务（8787–8807 回退），与网页版共用同一引擎与前端
            let state = AppState::new();
            let (port, _handle) = tauri::async_runtime::block_on(start_server(state, &default_ports()))
                .map_err(|e| format!("核心服务启动失败：{e}"))?;
            let url = format!("http://127.0.0.1:{port}");
            let parsed: tauri::Url = url.parse().map_err(|e| format!("URL 解析失败：{e}"))?;

            let app_handle = app.handle().clone();
            let win = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(parsed))
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

            // 关闭按钮 → 最小化到托盘（真正退出请用托盘右键「退出」）
            win.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    if let Some(w) = app_handle.get_webview_window("main") {
                        let _ = w.hide();
                    }
                }
            });

            build_tray(&app.handle().clone())?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("WebFix Studio 桌面版运行失败");
}