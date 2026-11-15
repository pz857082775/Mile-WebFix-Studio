# WebFix Studio · 构建与运行（Rust 多平台版）

网页版与桌面版共用 **同一套核心引擎（Rust）** 与 **同一套前端（React + TypeScript + TailwindCSS）**，
界面与功能完全一致。桌面版只是用 Tauri 2 的 WebView 加载核心服务提供的同一前端。

```
webfix-rust/
├─ crates/core      核心引擎 + axum 服务（lib: webfix_core，bin: webfix-server）
├─ crates/desktop   Tauri 2 桌面壳（bin: webfix-desktop，Windows / Linux / macOS）
├─ web              统一前端（React 18 + TS + Tailwind 3，Vite 构建到 web/dist）
├─ assets           知识库 knowledge.json（261 条，编译期内嵌）
└─ scripts          smoke.mjs（API 全量冒烟）/ dump-kb.mjs
```

## 1. 前置依赖

- Rust stable（≥ 1.80，建议 rustup 安装）
- Node.js ≥ 18（构建前端）
- 桌面版额外依赖：
  - **Windows**：WebView2 Runtime（Win10/11 通常已自带）；MSVC 构建工具
  - **Linux**：`libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev patchelf`
  - **macOS**：Xcode Command Line Tools（`xcode-select --install`）

## 2. 构建统一前端

```bash
cd web
npm install
npm run build      # 产物输出到 web/dist（核心服务会内嵌/读取它）
```

## 3. 网页版

```bash
# 开发调试（前端热更新，代理到 8787）
cd web && npm run dev

# 生产运行（核心服务直接提供 web/dist）
cargo run -p webfix-core --bin webfix-server --release
# 默认 http://127.0.0.1:8787（8787–8807 自动回退），可用 PORT 环境变量指定
# 加 --open 自动打开浏览器
```

`webfix-server` 是单文件可执行程序，内嵌前端与知识库，拷贝到任意机器即可运行网页版。

## 4. 桌面版（Tauri 2）

桌面壳在进程内启动核心服务（8787–8807 回退），WebView 加载 `http://127.0.0.1:<port>`，
因此与网页版同源、同界面、同功能；单实例运行，外部链接自动交给系统浏览器。

```bash
# 安装 Tauri CLI（一次即可）
cargo install tauri-cli --version "^2" --locked

# 先构建前端（见第 2 步），再打包桌面版
cd crates/desktop
cargo tauri build
```

产物位于 `target/release/bundle/`：

| 平台 | 安装包 |
| --- | --- |
| Windows | `nsis/*.exe`（推荐）、`msi/*.msi` |
| Linux | `deb/*.deb`、`appimage/*.AppImage` |
| macOS | `dmg/*.dmg`、`macos/*.app` |

仅本机调试桌面壳（不打包）：

```bash
cargo run -p webfix-desktop
```

## 5. 测试

```bash
cargo test -p webfix-core          # 核心单元测试（引擎/编排/知识库计数等）
node scripts/smoke.mjs             # 需先启动服务：API 全量冒烟（111 项）
```

## 6. 持续集成

`.github/workflows/build.yml` 在 Ubuntu / Windows / macOS 三平台自动构建前端 + 桌面安装包，
产物上传为 Artifacts。推送到 main/master 或手动触发即可。

## 版权

© 米乐 · v1.2.1 · 微信 mileai8 · 邮箱 857082775@qq.com
仅可扫描自有或已获授权的网站；AI 密钥仅存本机、只发往你配置的 API 地址；零成本模式绝不联网大模型。