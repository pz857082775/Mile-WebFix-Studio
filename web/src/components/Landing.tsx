import { PAGE_META } from '../lib/format';
import { PageIcon } from '../lib/icons';
import type { PageId } from '../lib/types';

const FEATURES: { id: PageId; color: string; desc: string }[] = [
  { id: 'scan', color: 'cat-blue', desc: '30+ 启发式规则 + 响应头审计 + 敏感路径探测' },
  { id: 'crawl', color: 'cat-green', desc: 'BFS + sitemap 自动发现全站路由，聚合去重' },
  { id: 'inspect', color: 'cat-cyan', desc: '内嵌浏览器登录巡检，实时抓控制台 / 网络 / DOM' },
  { id: 'fix', color: 'cat-purple', desc: '七字段 → 四件套：修复 + 验证 + 防护 + git diff' },
  { id: 'kb', color: 'cat-orange', desc: '261 条规则覆盖 OWASP / API / CWE / 逻辑 / 数据 / 算法' },
  { id: 'ai', color: 'cat-pink', desc: '零成本默认；8 家大模型增强挖掘，密钥仅存本机' },
  { id: 'ext', color: 'cat-geek', desc: '智能体 / SKILL / MCP / Bot 统一注册、权限、审计' },
  { id: 'orch', color: 'cat-red', desc: '串行 / 并行 / 条件编排，超时重试与错误策略' },
];

const PREVIEW: { sev: string; label: string; title: string; rule: string }[] = [
  { sev: 'critical', label: '严重', title: '表单缺少 CSRF 防护', rule: 'SEC-CSRF-01' },
  { sev: 'high', label: '高危', title: '响应头缺失 Content-Security-Policy', rule: 'SEC-HDR-03' },
  { sev: 'medium', label: '中危', title: '图片缺少 alt 属性', rule: 'UI-ALT-01' },
];

const METERS: { id: PageId; tile: string; name: string; sub: string; pill: string; bar: string; label: string; pct: string }[] = [
  { id: 'kb', tile: 'bg-[#d8e9fd] text-[#1677ff]', name: 'BUG / 漏洞知识库', sub: 'OWASP Top10 · API Top10 · CWE Top25 · 逻辑 / 数据 / 算法', pill: '261 条', bar: '#1677ff', label: '覆盖 8/8 类', pct: '100%' },
  { id: 'ext', tile: 'bg-[#e5dbfb] text-[#722ed1]', name: '扩展与协同编排', sub: '智能体 · SKILL · MCP · Bot ／ 串行 · 并行 · 条件', pill: '4 类', bar: '#722ed1', label: '3 种编排模式', pct: '100%' },
  { id: 'crawl', tile: 'bg-[#c8eed7] text-[#00b42a]', name: '多平台同源', sub: '网页版 + 桌面版，界面与功能完全一致', pill: '3 平台', bar: '#00b42a', label: 'Win / Linux / macOS', pct: '100%' },
];

function SectionHead({ title, sub, icon }: { title: string; sub?: string; icon: string }) {
  return (
    <div className="flex items-center gap-2 mb-4">
      <svg viewBox="0 0 24 24" className="w-4 h-4 text-ink-3" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d={icon} /></svg>
      <span className="text-sm font-semibold text-ink-2">{title}</span>
      {sub && <span className="text-xs text-ink-3">{sub}</span>}
    </div>
  );
}

export function Landing({ onOpen, onChangelog }: { onOpen: (id: PageId) => void; onChangelog: () => void }) {
  return (
    <div className="h-full overflow-auto bg-white">
      <div className="max-w-[1120px] mx-auto px-8 pt-10 pb-6">
        <div className="flex items-center gap-10">
          <div className="flex-1 min-w-0 animate-paneIn">
            <img src="/logo.png" alt="WebFix Studio LOGO" className="w-[92px] h-[92px] rounded-[24px] shadow-dialog mb-6" />
            <div className="inline-flex items-center gap-2 px-3 h-7 rounded-full bg-accent-soft text-accent text-xs font-medium">
              <span className="w-1.5 h-1.5 rounded-full bg-accent animate-blink" />
              WebFix Studio <button className="font-semibold hover:underline cursor-pointer" title="查看历史更新记录" onClick={onChangelog}>v1.2.1</button> · 网页 + 桌面同源 · 三平台
            </div>
            <h1 className="mt-5 text-[56px] leading-none font-black tracking-tight text-ink select-none">米乐大神</h1>
            <p className="mt-4 text-[17px] text-ink-2 font-medium">网页 BUG 与漏洞，从发现到修复一次闭环。</p>
            <p className="mt-2 text-[13px] text-ink-3 leading-relaxed max-w-[520px]">
              自动体检、全站爬行、人工巡检 + AI 自动操作识别；261 条安全规则零成本开箱即用，
              四件套修复方案可直接提交。
            </p>
            <div className="mt-6 flex items-center gap-3">
              <button className="btn !h-10 !px-5" onClick={() => onOpen('scan')}>立即开始体检</button>
              <button className="btn-ghost !h-10 !px-5" onClick={() => onOpen('inspect')}>进入人工巡检</button>
            </div>
            <div className="mt-5 flex flex-wrap gap-x-5 gap-y-2 text-[11px] text-ink-3">
              <span className="inline-flex items-center gap-1.5">
                <svg viewBox="0 0 24 24" className="w-3.5 h-3.5" fill="none" stroke="currentColor" strokeWidth="1.8"><path d="M12 2 4 5v6c0 5 3.4 9.4 8 11 4.6-1.6 8-6 8-11V5l-8-3Z" /></svg>
                仅扫自有或授权站点
              </span>
              <span className="inline-flex items-center gap-1.5">
                <svg viewBox="0 0 24 24" className="w-3.5 h-3.5" fill="none" stroke="currentColor" strokeWidth="1.8"><path d="M14 10a4 4 0 1 0-4 4c.6 0 1.2-.1 1.7-.4L13 15h2v2h2v2h3v-3l-5.4-5.4c.3-.5.4-1 .4-1.6Z" /></svg>
                AI 密钥仅存本机
              </span>
              <span className="inline-flex items-center gap-1.5">
                <svg viewBox="0 0 24 24" className="w-3.5 h-3.5" fill="none" stroke="currentColor" strokeWidth="1.8"><path d="M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20Zm0 5v10m-3-7h6m-6 3h6" /></svg>
                零成本模式不联网大模型
              </span>
            </div>
          </div>

          <div className="hidden xl:block w-[380px] shrink-0 animate-paneIn" style={{ animationDelay: '120ms' }}>
            <div className="rounded-[10px] border border-line shadow-dialog overflow-hidden bg-white">
              <div className="dlg-title !h-9">
                <img src="/logo.png" alt="WebFix Studio" className="w-4 h-4 rounded" />
                <span className="text-xs font-medium text-ink-2 flex-1">体检报告 · demo</span>
                <span className="w-1.5 h-1.5 rounded-full bg-ok animate-blink" />
              </div>
              <div className="p-3 space-y-2">
                {PREVIEW.map((r) => (
                  <div key={r.rule} className="flex items-center gap-2 rounded-lg border border-line px-3 py-2">
                    <span className={`badge ${r.sev}`}>{r.label}</span>
                    <span className="flex-1 min-w-0 text-xs text-ink-2 truncate">{r.title}</span>
                    <span className="text-[10px] text-ink-3 font-mono">{r.rule}</span>
                  </div>
                ))}
              </div>
              <div className="dlg-footer !py-2.5">
                <span className="text-[11px] text-ink-3 flex-1">3 个问题 · 128ms</span>
                <span className="text-xs font-medium text-accent">生成修复方案</span>
              </div>
            </div>
            <p className="text-[11px] text-ink-3 mt-2 text-center">体检报告预览 · 零成本规则引擎</p>
          </div>
        </div>

        <div className="mt-12 animate-paneIn" style={{ animationDelay: '160ms' }}>
          <SectionHead title="功能中心" sub="点击卡片打开对应页签" icon="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7Z" />
          <div className="grid grid-cols-4 gap-4">
            {FEATURES.map((f, i) => (
              <button
                key={f.id}
                className={`cat-card ${f.color} animate-paneIn`}
                style={{ animationDelay: `${200 + i * 50}ms` }}
                onClick={() => onOpen(f.id)}
              >
                <span className="cat-tile"><PageIcon id={f.id} className="w-5 h-5" /></span>
                <span className="cat-name">{PAGE_META[f.id][0]}</span>
                <span className="cat-desc">{f.desc}</span>
              </button>
            ))}
          </div>
        </div>

        <div className="mt-10 animate-paneIn" style={{ animationDelay: '320ms' }}>
          <SectionHead title="能力概览" icon="M4 20V10m6 10V4m6 16v-7m4 7H2" />
          <div className="card divide-y divide-line">
            {METERS.map((m) => (
              <div key={m.name} className="flex items-center gap-4 px-5 py-4">
                <span className={`w-10 h-10 rounded-[10px] flex items-center justify-center shrink-0 ${m.tile}`}>
                  <PageIcon id={m.id} className="w-5 h-5" />
                </span>
                <div className="flex-1 min-w-0">
                  <div className="text-sm font-medium text-ink">{m.name}</div>
                  <div className="text-xs text-ink-3 mt-0.5 truncate">{m.sub}</div>
                </div>
                <span className="hidden md:inline-flex items-center px-3 h-8 rounded-md bg-side text-xs font-medium text-ink-2 shrink-0">{m.pill}</span>
                <div className="hidden lg:block w-[160px] shrink-0">
                  <div className="flex items-center justify-between text-[11px] mb-1.5">
                    <span className="text-ink-3">{m.label}</span>
                    <span className="font-semibold" style={{ color: m.bar }}>{m.pct}</span>
                  </div>
                  <div className="meter"><i style={{ width: m.pct, background: m.bar }} /></div>
                </div>
              </div>
            ))}
          </div>
        </div>

        <div className="mt-8 rounded-pane bg-accent-soft/60 border border-accent/20 p-4 flex items-start gap-3 animate-paneIn" style={{ animationDelay: '400ms' }}>
          <svg viewBox="0 0 24 24" className="w-5 h-5 text-accent shrink-0 mt-0.5" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round"><path d="M12 2 4 5v6c0 5 3.4 9.4 8 11 4.6-1.6 8-6 8-11V5l-8-3Zm-3 10 2.2 2.2L15.5 10" /></svg>
          <div>
            <div className="text-sm font-semibold text-ink mb-1">安全承诺</div>
            <div className="text-xs text-ink-2 leading-relaxed">
              仅扫描自有或已获授权网站 · AI 密钥仅存本机、只发往你配置的 API 地址 · 零成本模式绝不联网大模型
            </div>
          </div>
        </div>

        <div className="mt-6 flex items-center justify-between text-[11px] text-ink-3">
          <span>© 米乐 · <button className="hover:text-accent cursor-pointer" title="查看历史更新记录" onClick={onChangelog}>v1.2.1</button> · 微信 mileai8 · 857082775@qq.com</span>
          <span>仅可扫描自有或已获授权的网站</span>
        </div>
      </div>
    </div>
  );
}