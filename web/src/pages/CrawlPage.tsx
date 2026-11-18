import { Fragment, useEffect, useRef, useState } from 'react';
import { api, download, isLocalUrl } from '../lib/api';
import { CAT, SEV, summaryOf } from '../lib/format';
import type { CrawlJob, Finding, Rule } from '../lib/types';
import { FindingCard } from '../components/FindingCard';
import { FindingFilters, emptyFindingFilter, matchesFinding } from '../components/FindingFilters';
import { SummaryBar } from '../components/SummaryBar';
import { useToast } from '../components/Toast';
import { PageHeader } from '../components/PageHeader';
import { Section } from '../components/ui';

export function CrawlPage({ onFix }: { onFix: (f: Finding, url?: string) => void }) {
  const toast = useToast();
  const [url, setUrl] = useState(`${location.origin}/demo`);
  const [maxPages, setMaxPages] = useState(30);
  const [maxDepth, setMaxDepth] = useState(3);
  const [authorized, setAuthorized] = useState(false);
  const [useSession, setUseSession] = useState(true);
  const [cookie, setCookie] = useState('');
  const [authorization, setAuthorization] = useState('');
  const [job, setJob] = useState<CrawlJob | null>(null);
  const [filter, setFilter] = useState(emptyFindingFilter);
  const [busy, setBusy] = useState(false);
  const [aiEnabled, setAiEnabled] = useState(false);
  const [aiBusy, setAiBusy] = useState(false);
  const [aiStatus, setAiStatus] = useState('');
  const [rules, setRules] = useState<Rule[]>([]);
  const [expanded, setExpanded] = useState<string | null>(null);
  const timer = useRef<number>(0);

  useEffect(() => {
    api<{ config: { enabled: boolean } }>('/api/ai/config').then((d) => setAiEnabled(d.config.enabled)).catch(() => {});
    api<{ rules: Rule[] }>('/api/rules').then((d) => setRules(d.rules)).catch(() => {});
    return () => window.clearInterval(timer.current);
  }, []);
  useEffect(() => { if (isLocalUrl(url)) setAuthorized(true); }, [url]);

  const start = async () => {
    if (!url.trim()) return toast('请填写目标网址');
    setBusy(true);
    setJob(null);
    try {
      const { id } = await api<{ id: string }>('/api/crawl', { url: url.trim(), maxPages, maxDepth, authorized, useSession, cookie: cookie.trim(), authorization: authorization.trim() });
      timer.current = window.setInterval(async () => {
        try {
          const j = await api<CrawlJob>('/api/crawl-status?id=' + id);
          setJob(j);
          if (j.status !== 'running') {
            window.clearInterval(timer.current);
            setBusy(false);
            if (j.status === 'error') toast('爬行失败：' + (j.error || ''));
          }
        } catch { /* 轮询瞬时失败忽略 */ }
      }, 1000);
    } catch (e: any) {
      toast(e.message);
      setBusy(false);
    }
  };

  const briefPrompt = (f: Finding): string => {
    const rule = rules.find((r) => r.id === f.ruleId);
    const hint = rule
      ? (rule.fix.backend.split('\n').find((l) => l.trim() && !l.trim().startsWith('//')) || rule.fix.frontend.split('\n')[0] || '见知识库模板')
      : '结合证据定位根因后最小改动修复';
    const lines = [
      `【问题】${f.title}（${SEV[f.severity] || f.severity} · ${CAT[f.category] || f.category}）`,
      `【页面】${f.url || (f.urls || []).join(' , ') || '（未提供）'}`,
      `【描述】${(f.rootCause || rule?.rootCause || '').slice(0, 200)}`,
    ];
    if (f.evidence) lines.push(`【证据】${f.evidence.split('\n')[0].slice(0, 200)}`);
    lines.push(`【修复】${hint.slice(0, 140)}`);
    lines.push('直接输出：最简修复代码（前端/后端分开）+ 可提交的 git diff。不要解释概念。');
    return lines.join('\n');
  };

  const exportPrompts = () => {
    if (!job) return;
    const text = job.findings.map((f, i) => `===== ${i + 1}. ${f.title} =====\n${briefPrompt(f)}`).join('\n\n');
    download('webfix-prompts.txt', text);
    toast(`已导出 ${job.findings.length} 条精简修复提示词`);
  };

  const exportPatch = async () => {
    if (!job) return;
    const items = await Promise.all(job.findings.map(async (f) => {
      const r = await api<{ deliverables: { gitDiff: string } }>('/api/fix', { finding: f, stack: {} });
      return { title: f.title, severity: f.severity, gitDiff: r.deliverables.gitDiff };
    }));
    const text = await api<string>('/api/patch', { items });
    download('webfix-crawl.patch', text);
    toast('已下载合并补丁（路径需按实际项目调整）');
  };

  const deep = async () => {
    if (!job) return;
    if (!aiEnabled) { toast('AI 增强未启用：请到「AI 设置」配置并启用'); return; }
    setAiBusy(true);
    setAiStatus('AI 深度挖掘中…');
    try {
      const d = await api<{ findings: Finding[]; model: string }>('/api/ai/analyze', {
        kind: 'crawl', url: url.trim(), pages: job.pages.map((p) => p.url), findings: job.findings,
      });
      const titles = new Set(job.findings.map((f) => f.title));
      const added = d.findings.filter((f) => !titles.has(f.title));
      const merged = [...job.findings, ...added];
      setJob({ ...job, findings: merged, summary: summaryOf(merged) });
      setAiStatus(`AI 深度挖掘完成：新增 ${added.length} 条（模型 ${d.model}）`);
    } catch (e: any) {
      setAiStatus('AI 挖掘失败：' + e.message);
    } finally {
      setAiBusy(false);
    }
  };

  const pct = job ? Math.min(100, Math.round((job.scanned / Math.max(1, job.maxPages)) * 100)) : 0;

  return (
    <div className="p-6 max-w-[1080px] mx-auto">
      <PageHeader id="crawl" />
      <Section
        title="全站爬行"
        desc="自动发现同域路由 + sitemap，逐页体检并聚合去重，可导出精简修复提示词与合并补丁。"
        actions={<button className="btn !h-8 !text-xs" disabled={busy} onClick={start}>{busy ? '爬行中…' : '开始全站爬行'}</button>}
      >
        <div className="flex gap-3 items-end flex-wrap">
          <label className="flex-1 min-w-[260px]">
            <span className="label">起始网址</span>
            <input className="input" type="url" value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://你的网站.example" />
          </label>
          <label className="w-28">
            <span className="label">最多页数</span>
            <input className="input" type="number" min={1} max={100} value={maxPages} onChange={(e) => setMaxPages(+e.target.value || 30)} />
          </label>
          <label className="w-28">
            <span className="label">最大深度</span>
            <input className="input" type="number" min={1} max={6} value={maxDepth} onChange={(e) => setMaxDepth(+e.target.value || 3)} />
          </label>
        </div>
        <label className="flex items-center gap-2 mt-3 text-[13px] text-ink-2 cursor-pointer">
          <input type="checkbox" className="accent-[#1677ff]" checked={authorized} onChange={(e) => setAuthorized(e.target.checked)} />
          我拥有该网站或已获得测试授权（本机演示地址无需勾选）
        </label>
        <div className="mt-3 rounded-lg border border-line bg-panel px-3 py-2.5">
          <label className="flex items-center gap-2 text-[13px] text-ink-2 cursor-pointer">
            <input type="checkbox" className="accent-[#1677ff]" checked={useSession} onChange={(e) => setUseSession(e.target.checked)} />
            复用人工巡检登录态（先在「人工巡检」内嵌浏览器登录，爬行即可扫登录墙后的页面）
          </label>
          <div className="flex gap-3 mt-2 flex-wrap">
            <label className="flex-1 min-w-[220px]">
              <span className="label">手动 Cookie（可选，优先于登录态）</span>
              <input className="input" value={cookie} onChange={(e) => setCookie(e.target.value)} placeholder="session=abc; token=…" />
            </label>
            <label className="flex-1 min-w-[220px]">
              <span className="label">手动 Authorization（可选）</span>
              <input className="input" value={authorization} onChange={(e) => setAuthorization(e.target.value)} placeholder="Bearer eyJ…" />
            </label>
          </div>
        </div>
        {job && job.status === 'running' && (
          <div className="mt-4 animate-paneIn">
            <div className="meter"><i className="bg-accent transition-all duration-500" style={{ width: `${Math.max(6, pct)}%` }} /></div>
            <p className="text-xs text-ink-3 mt-1.5">已扫描 {job.scanned} 页 · 发现 {job.findings.length} 个问题</p>
          </div>
        )}
        {job && job.status === 'error' && <p className="text-[13px] text-danger mt-2">爬行失败：{job.error}</p>}
      </Section>

      {job && job.status !== 'running' && (
        <>
          <SummaryBar summary={job.summary} label="全站累计发现" />
          <div className="flex items-center gap-3 mb-4 flex-wrap">
            <button className="btn-ghost" onClick={exportPrompts}>导出全部精简修复提示词</button>
            <button className="btn-ghost" onClick={exportPatch}>导出合并补丁 .patch</button>
            <button className="btn-ghost" disabled={aiBusy} onClick={deep}>{aiBusy ? 'AI 挖掘中…' : 'AI 深度挖掘'}</button>
            {aiStatus && <span className="text-xs text-ink-3">{aiStatus}</span>}
          </div>
          <FindingFilters value={filter} onChange={setFilter} total={job.findings.length} count={job.findings.filter(f => matchesFinding(f, filter)).length} />
          <Section title={`已扫描页面 · ${job.pages.length}`} desc="点击页面展开该页问题，问题列表随筛选条件变化" bodyClass="px-5 py-3">
            <div className="max-h-[420px] overflow-auto">
              <table className="w-full text-xs">
                <thead><tr className="text-ink-3 text-left"><th className="py-1.5 font-medium">URL</th><th className="w-16 font-medium">状态</th><th className="w-20 font-medium">问题数</th></tr></thead>
                <tbody>
                  {job.pages.map((p) => (
                    <Fragment key={p.url}>
                      <tr
                        className={`border-t border-line cursor-pointer hover:bg-panel ${expanded === p.url ? 'bg-panel' : ''}`}
                        onClick={() => setExpanded(expanded === p.url ? null : p.url)}
                      >
                        <td className="py-1.5 pr-2 truncate max-w-[520px]" title={p.url}>
                          <span className="text-ink-3 mr-1.5">{expanded === p.url ? '▾' : '▸'}</span>{p.url}
                        </td>
                        <td>{p.error ? <span className="text-danger">错误</span> : p.status}</td>
                        <td>
                          {p.count > 0
                            ? <span className="badge high">{p.count}</span>
                            : <span className="text-ink-3">0</span>}
                        </td>
                      </tr>
                      {expanded === p.url && (
                        <tr className="border-t border-line bg-panel">
                          <td colSpan={3} className="px-3 py-2">
                            {p.error && <p className="text-danger mb-1">扫描失败：{p.error}</p>}
                            {(p.findings && p.findings.filter(f => matchesFinding({ ...f, url: p.url }, filter)).length > 0) ? (
                              <div className="space-y-1.5">
                                {p.findings.filter(f => matchesFinding({ ...f, url: p.url }, filter)).map((f, i) => (
                                  <div key={i} className="flex items-start gap-2">
                                    <span className={`badge shrink-0 ${f.severity === 'critical' || f.severity === 'high' ? 'high' : f.severity === 'medium' ? 'medium' : 'cat'}`}>
                                      {SEV[f.severity] || f.severity}
                                    </span>
                                    <div className="min-w-0">
                                      <span className="text-ink-1">{f.title}</span>
                                      <span className="text-ink-3 ml-2">{CAT[f.category] || f.category}</span>
                                      {f.evidence && <p className="text-ink-3 break-all mt-0.5">{String(f.evidence).split('\n')[0].slice(0, 160)}</p>}
                                    </div>
                                    <button
                                      className="btn-ghost !h-6 !text-[11px] shrink-0 ml-auto"
                                      onClick={(e) => { e.stopPropagation(); onFix({ ...f, url: p.url }, p.url); }}
                                    >修复</button>
                                  </div>
                                ))}
                              </div>
                            ) : (
                              <p className="text-ink-3">该页没有符合筛选条件的问题。</p>
                            )}
                          </td>
                        </tr>
                      )}
                    </Fragment>
                  ))}
                </tbody>
              </table>
            </div>
          </Section>
          {job.findings.filter(f => matchesFinding(f, filter)).map((f, i) => (
            <FindingCard key={(f.ruleId || f.title) + i} f={f} index={i} onFix={(x) => onFix(x, x.url || (x.urls || [])[0] || '')} />
          ))}
        </>
      )}
    </div>
  );
}