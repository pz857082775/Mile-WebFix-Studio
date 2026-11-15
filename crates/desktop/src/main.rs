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

/// 在系统文件管理器中打开更新包所在文件夹（Windows 选中文件，macOS reveal，Linux 打开目录）
#[tauri::command]
fn reveal_in_folder(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    let dir = if p.is_dir() {
        p.to_path_buf()
    } else {
        p.parent().map(|x| x.to_path_buf()).unwrap_or_else(|| p.to_path_buf())
    };
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let mut c = std::process::Command::new("explorer");
        if p.is_file() {
            c.raw_arg(format!("/select,\"{}\"", path));
        } else {
            c.arg(&dir);
        }
        c.spawn().map_err(|e| format!("打开文件夹失败：{e}"))?;
    }
    #[cfg(target_os = "macos")]
    {
        let mut c = std::process::Command::new("open");
        if p.is_file() { c.arg("-R").arg(&path); } else { c.arg(&dir); }
        c.spawn().map_err(|e| format!("打开文件夹失败：{e}"))?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open").arg(&dir).spawn().map_err(|e| format!("打开文件夹失败：{e}"))?;
    }
    Ok(())
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
    // 先启动核心服务拿到端口，再把该地址登记为应用自身 origin（frontendDist）。
    // 否则 Tauri 把 http://127.0.0.1 视为 remote origin 而禁用 IPC，
    // 导致自定义标题栏窗口控制与 reveal_in_folder 等命令全部失效。
    let state = AppState::new();
    let (port, _server) = tauri::async_runtime::block_on(start_server(state, &default_ports()))
        .expect("核心服务启动失败");
    let url = format!("http://127.0.0.1:{port}");
    let parsed: tauri::Url = url.parse().expect("URL 解析失败");

    let mut context = tauri::generate_context!();
    context.config_mut().build.frontend_dist =
        Some(tauri::utils::config::FrontendDist::Url(parsed.clone()));

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            show_main(app);
        }))
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![reveal_in_folder])
        .setup(move |app| {
            let app_handle = app.handle().clone();
            let win = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(parsed.clone()))
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
        .run(context)
        .expect("WebFix Studio 桌面版运行失败");
}