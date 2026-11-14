import { useEffect, useRef, useState } from 'react';
import { api, download, isLocalUrl } from '../lib/api';
import { CAT, SEV, summaryOf } from '../lib/format';
import type { CrawlJob, Finding, Rule } from '../lib/types';
import { FindingCard } from '../components/FindingCard';
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
  const [job, setJob] = useState<CrawlJob | null>(null);
  const [busy, setBusy] = useState(false);
  const [aiEnabled, setAiEnabled] = useState(false);
  const [aiBusy, setAiBusy] = useState(false);
  const [aiStatus, setAiStatus] = useState('');
  const [rules, setRules] = useState<Rule[]>([]);
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
      const { id } = await api<{ id: string }>('/api/crawl', { url: url.trim(), maxPages, maxDepth, authorized });
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
          <Section title={`已扫描页面 · ${job.pages.length}`} bodyClass="px-5 py-3">
            <div className="max-h-52 overflow-auto">
              <table className="w-full text-xs">
                <thead><tr className="text-ink-3 text-left"><th className="py-1.5 font-medium">URL</th><th className="w-16 font-medium">状态</th><th className="w-16 font-medium">问题</th></tr></thead>
                <tbody>
                  {job.pages.map((p) => (
                    <tr key={p.url} className="border-t border-line">
                      <td className="py-1.5 pr-2 truncate max-w-[560px]" title={p.url}>{p.url}</td>
                      <td>{p.status}</td>
                      <td>{p.findings}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </Section>
          {job.findings.map((f, i) => (
            <FindingCard key={(f.ruleId || f.title) + i} f={f} index={i} onFix={(x) => onFix(x, x.url || (x.urls || [])[0] || '')} />
          ))}
        </>
      )}
    </div>
  );
}