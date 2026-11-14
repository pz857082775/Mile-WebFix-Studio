import { useEffect, useState } from 'react';
import { api, isLocalUrl } from '../lib/api';
import { summaryOf } from '../lib/format';
import type { Finding, ScanResult } from '../lib/types';
import { FindingCard } from '../components/FindingCard';
import { SummaryBar } from '../components/SummaryBar';
import { useToast } from '../components/Toast';
import { PageHeader } from '../components/PageHeader';
import { Section } from '../components/ui';

export function ScanPage({ onFix }: { onFix: (f: Finding, url?: string) => void }) {
  const toast = useToast();
  const [url, setUrl] = useState(`${location.origin}/demo`);
  const [authorized, setAuthorized] = useState(false);
  const [probe, setProbe] = useState(false);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState('');
  const [data, setData] = useState<ScanResult | null>(null);
  const [aiEnabled, setAiEnabled] = useState(false);
  const [aiBusy, setAiBusy] = useState(false);
  const [aiStatus, setAiStatus] = useState('');

  useEffect(() => {
    api<{ config: { enabled: boolean } }>('/api/ai/config').then((d) => setAiEnabled(d.config.enabled)).catch(() => {});
  }, []);
  useEffect(() => { if (isLocalUrl(url)) setAuthorized(true); }, [url]);

  const run = async () => {
    if (!url.trim()) return toast('请填写目标网址');
    setBusy(true);
    setStatus('体检中：抓取页面 → 规则匹配 → 响应头审计 → 链接提取…');
    try {
      const r = await api<ScanResult>('/api/scan', { url: url.trim(), authorized, probePaths: probe });
      setData(r);
      setStatus(`体检完成：${r.findings.length} 个问题 · 耗时 ${r.durationMs}ms · HTTP ${r.status}`);
    } catch (e: any) {
      setStatus('体检失败：' + e.message);
    } finally {
      setBusy(false);
    }
  };

  const deep = async () => {
    if (!data) return;
    if (!aiEnabled) { toast('AI 增强未启用：请到「AI 设置」配置并启用，或继续零成本模式'); return; }
    setAiBusy(true);
    setAiStatus('AI 深度挖掘中…（通常 10-60 秒，证据已发送给你配置的模型）');
    try {
      const d = await api<{ findings: Finding[]; model: string }>('/api/ai/analyze', {
        kind: 'scan', url: data.finalUrl, findings: data.findings, headers: data.headers, html: data.html,
      });
      const titles = new Set(data.findings.map((f) => f.title));
      const added = d.findings.filter((f) => !titles.has(f.title));
      const merged = [...data.findings, ...added];
      setData({ ...data, findings: merged, summary: summaryOf(merged) });
      setAiStatus(`AI 深度挖掘完成：新增 ${added.length} 条（模型 ${d.model}）`);
      toast(`AI 深度挖掘：+${added.length} 条`);
    } catch (e: any) {
      setAiStatus('AI 挖掘失败：' + e.message);
    } finally {
      setAiBusy(false);
    }
  };

  return (
    <div className="p-6 max-w-[1080px] mx-auto">
      <PageHeader id="scan" />
      <Section
        title="单页体检"
        desc="抓取页面 → 规则匹配 → 响应头审计 → 链接提取；可选敏感路径探测（仅限已授权站点）。"
        actions={<button className="btn !h-8 !text-xs" disabled={busy} onClick={run}>{busy ? '体检中…' : '开始体检'}</button>}
      >
        <label className="block">
          <span className="label">目标网址</span>
          <input className="input" type="url" value={url} onChange={(e) => setUrl(e.target.value)}
            placeholder="https://你的网站.example" onKeyDown={(e) => e.key === 'Enter' && run()} />
        </label>
        <div className="flex flex-wrap gap-x-6 gap-y-2 mt-3 text-[13px] text-ink-2">
          <label className="flex items-center gap-2 cursor-pointer">
            <input type="checkbox" className="accent-[#1677ff]" checked={authorized} onChange={(e) => setAuthorized(e.target.checked)} />
            我拥有该网站或已获得测试授权（本机演示地址无需勾选）
          </label>
          <label className="flex items-center gap-2 cursor-pointer">
            <input type="checkbox" className="accent-[#1677ff]" checked={probe} onChange={(e) => setProbe(e.target.checked)} />
            启用敏感路径探测（.env / .git / 备份文件等，仅限已授权站点）
          </label>
        </div>
        <p className="text-xs text-ink-3 mt-2.5">
          内置演示靶场：<a className="text-accent hover:underline" href="/demo" target="_blank" rel="noopener">/demo</a>（多页站点，含登录墙 /demo/admin）
        </p>
        {status && <p className="text-[13px] text-ink-2 mt-2 animate-paneIn">{status}</p>}
      </Section>

      {data && (
        <>
          <SummaryBar summary={data.summary} label="体检发现" />
          <div className="flex items-center gap-3 mb-4">
            <button className="btn-ghost" disabled={aiBusy} onClick={deep}>
              {aiBusy ? 'AI 挖掘中…' : 'AI 深度挖掘（逻辑 / 越权 / 资损等深层问题）'}
            </button>
            {aiStatus && <span className="text-xs text-ink-3">{aiStatus}</span>}
          </div>
          {data.findings.map((f, i) => (
            <FindingCard key={(f.ruleId || f.title) + i} f={f} index={i} onFix={(x) => onFix(x, data.finalUrl)} />
          ))}
          {data.findings.length === 0 && <div className="card p-8 text-center text-ink-3 text-sm">未发现已知问题，可尝试 AI 深度挖掘或人工巡检</div>}
        </>
      )}
    </div>
  );
}