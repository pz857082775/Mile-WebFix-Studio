fn main() {
    // 注册自定义命令以自动生成 allow-/deny- 权限（capability 中需显式授予 allow-reveal-in-folder）
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&["reveal_in_folder"])),
    )
    .expect("failed to run tauri-build");
}