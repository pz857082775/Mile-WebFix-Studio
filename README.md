<p align="center">
  <img src="assets/logo.png" width="132" alt="WebFix Studio LOGO" />
</p>

<h1 align="center">WebFix Studio</h1>

<p align="center">
  <b>网页 BUG 与漏洞，从发现到修复一次闭环。</b><br/>
  一键体检 · 全站爬行 · 人工巡检 + AI 自动操作识别 · 261 条安全规则 · 四件套修复方案
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-1.1.0-1677ff" alt="version" />
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS%20%7C%20Web-00b42a" alt="platform" />
  <img src="https://img.shields.io/badge/Rust-axum-dea584" alt="rust" />
  <img src="https://img.shields.io/badge/React-18%20%2B%20TS%20%2B%20Tailwind-61dafb" alt="react" />
  <img src="https://img.shields.io/badge/安全规则-261%20条-722ed1" alt="rules" />
  <img src="https://img.shields.io/badge/%C2%A9-米乐-f53f3f" alt="copyright" />
</p>

---

## ✨ 简介

**WebFix Studio** 是一款面向普通开发者与安全爱好者的「网页 BUG / 漏洞巡检与修复工作台」。
它把 **发现 → 定位 → 修复 → 验证 → 防护** 做成一条闭环流水线：内置 261 条覆盖
OWASP Top10 / API Top10 / CWE Top25 / 逻辑 / 数据 / 算法 的安全规则，**零成本开箱即用**；
也可接入你自己的大模型做增强挖掘。网页版与桌面版**同源同界面同功能**，一次构建、三平台运行。

> 🛡️ **安全承诺**：仅扫描自有或已获授权的网站 · AI 密钥仅存本机、只发往你配置的 API 地址 · 零成本模式绝不联网大模型。

## 🧭 功能矩阵

| 模块 | 能力 | 亮点 |
| --- | --- | --- |
| 🔍 一键体检 | 单页 UI / 逻辑 / 功能 / 数据 / 算法 / 安全扫描 | 30+ 启发式规则 + 响应头审计 + 敏感路径探测 |
| 🕸️ 全站爬行 | BFS + sitemap 自动发现全站路由 | 聚合去重、逐页体检、进度可视、复用巡检登录态突破登录墙 |
| 🖱️ 人工巡检 | 内嵌浏览器登录巡检 | 实时抓控制台 / 网络 / DOM，AI 自动操作识别特效，登录态供爬行 / 体检复用 |
| 🛠️ 修复工作台 | 七字段 → 四件套 | 最小修复 + 验证步骤 + 防护代码 + 可提交 git diff |
| 📚 BUG 知识库 | 261 条规则与挖掘模板 | 越权 / 篡改 / SQL 注释 / 支付 / 秘钥 / 接口 全覆盖 |
| 🤖 AI 设置 | 零成本默认 + 8 家大模型增强 | 密钥仅存本机，服务端强制零成本不联网 |
| 🧩 扩展中心 | 智能体 / SKILL / MCP / Bot | 统一注册、配置、权限、审计 + 开源安全扩展市场 + AI 对话添加 + 拖拽导入 |
| 🔀 协同编排 | 串行 / 并行 / 条件流程 | 超时重试、错误策略、findings 聚合 + AI 对话自动挑选扩展并编排 |
| 🗔 桌面托盘 | 右键菜单 打开 / 设置 / 退出 | 左键唤起、关闭最小化到托盘、单实例聚焦 |
| 🕘 更新记录 | 点击版本号查看历史更新 | 仅公开摘要，内部实现与安全细节保密 |

## 💻 双形态 · 同源一致

- **网页版**：单文件 `webfix-server.exe`，内嵌前端与知识库，双击即用（`http://127.0.0.1:8787`）。
- **桌面版**：Tauri 2 原生壳（Windows / Linux / macOS），进程内启动同一核心服务，WebView 加载同一前端；
  原生无边框窗口、软件风格对话框、多标签页签（单个 / 批量关闭，全关回到「米乐大神」落地页）。

两者共用 **同一套 Rust 核心引擎** 与 **同一套 React + TypeScript + Tailwind 前端**，界面与功能完全一致。

## 🚀 快速开始

### 方式一：直接运行（推荐）

| 形态 | 文件 | 说明 |
| --- | --- | --- |
| 桌面 · 安装版 | [`WebFixStudio-1.1.0-x64-setup.exe`](release/desktop/WebFixStudio-1.1.0-x64-setup.exe) | NSIS 安装包（推荐） |
| 桌面 · 安装版 | [`WebFixStudio-1.1.0-x64.msi`](release/desktop/WebFixStudio-1.1.0-x64.msi) | MSI 安装包 |
| 桌面 · 便携版 | [`WebFixStudio-1.1.0-x64-portable.exe`](release/desktop/WebFixStudio-1.1.0-x64-portable.exe) | 免安装，双击运行 |
| 网页 · 单文件 | [`webfix-server.exe`](release/web/webfix-server.exe) | 双击后浏览器打开 `http://127.0.0.1:8787` |

内置演示靶场 `/demo`（含登录墙 `/demo/admin`），开箱即可体验全部功能。

### 方式二：从源码构建

```bash
# 1) 构建统一前端
cd web && npm install && npm run build

# 2) 网页版（单文件，内嵌前端 + 知识库）
cargo run -p webfix-core --bin webfix-server --release

# 3) 桌面版（Windows / Linux / macOS）
cd crates/desktop && cargo tauri build
```

详见 [`docs/BUILD.md`](docs/BUILD.md)。

## 🗂️ 目录结构

```
├─ crates/core        Rust 核心引擎 + axum 服务（扫描/爬行/修复/AI/代理/扩展/编排）
├─ crates/desktop     Tauri 2 桌面壳（三平台）
├─ web                统一前端（React 18 + TS + Tailwind 3）
├─ assets             知识库 knowledge.json（261 条）+ 品牌 LOGO
├─ scripts            API 全量冒烟 smoke.mjs / 知识库导出
├─ docs               构建与运行文档
└─ release            已构建交付物（桌面安装包 / 便携版 / 网页单文件）
```

## ✅ 质量保障

- 核心单元测试 **19/19** 通过（引擎 / 编排 / 代理 / 知识库计数对齐 / 扩展市场 / AI 编排）。
- 后端 API 全量冒烟 **111/111** 通过（对齐全部路由与权限拦截 + 扩展市场 / 导入 / AI 编排 + 巡检登录态复用突破登录墙）。
- 浏览器端到端实测通过（八大功能页 + 多标签批量关闭 + 落地页 + 扩展市场 / AI 编排 / 拖拽导入）。

## 🔒 安全与隐私

1. **授权扫描**：仅可扫描自有或已获授权的网站（内置授权勾选，localhost 豁免）。
2. **密钥本地化**：AI 密钥仅存本机系统用户目录（Windows `%APPDATA%\WebFixStudio`），只发往你配置的 API 地址。
3. **零成本硬隔离**：零成本模式由服务端强制不联网大模型，绝不产生费用。
4. **扩展沙箱**：模块 SKILL 限定目录、编排条件禁用 eval、外部端点校验 http(s)。
5. **配置持久化**：AI / 扩展 / 编排 / 审计 / 技能统一保存于系统每用户应用数据目录，重装 / 升级自动迁移旧配置，无需重新配置。

## © 版权与联系

> **Copyright © 米乐 · v1.1.0 · 保留所有权利。**
>
> - 作者：**米乐**
> - 微信：**mileai8**
> - 邮箱：**857082775@qq.com**
> - 版本：**1.1.0**
>
> 本软件（含源码、界面、知识库、图标与交付物）版权归 **米乐** 所有。
> 未经书面许可，不得用于商业再分发或去除版权标识。详见 [`COPYRIGHT.md`](COPYRIGHT.md)。

<p align="center">
  <img src="assets/logo.png" width="48" alt="logo" /><br/>
  <sub>© 米乐 · WebFix Studio v1.1.0 · 微信 mileai8 · 857082775@qq.com</sub>
</p>