import { useEffect, useRef, useState } from 'react';
import { api, isLocalUrl } from '../lib/api';
import { EV_LABEL, findingKey, fmtTime, summaryOf } from '../lib/format';
import type { CaptureEvent, Finding, InspectSession, Rule } from '../lib/types';
import { FindingCard } from '../components/FindingCard';
import { FindingFilters, emptyFindingFilter, matchesFinding } from '../components/FindingFilters';
import { SummaryBar } from '../components/SummaryBar';
import { useToast } from '../components/Toast';
import { PageHeader } from '../components/PageHeader';
import { Section } from '../components/ui';

declare global { interface Window { WfPilot: any } }

function b64urlDecode(s: string): string {
  s = s.replace(/-/g, '+').replace(/_/g, '/');
  while (s.length % 4) s += '=';
  return atob(s);
}

export function InspectPage({ rules, onFix }: { rules: Rule[]; onFix: (f: Finding, url?: string) => void }) {
  const toast = useToast();
  const [url, setUrl] = useState(`${location.origin}/demo`);
  const [authorized, setAuthorized] = useState(false);
  const [session, setSession] = useState<InspectSession | null>(null);
  const [events, setEvents] = useState<CaptureEvent[]>([]);
  const [findings, setFindings] = useState<Map<string, Finding>>(new Map());
  const [filter, setFilter] = useState(emptyFindingFilter);
  const [status, setStatus] = useState('');
  const [aiEnabled, setAiEnabled] = useState(false);
  const [aiBusy, setAiBusy] = useState(false);
  const [fx, setFx] = useState(true);
  const [allowSubmit, setAllowSubmit] = useState(false);
  const [pilotRunning, setPilotRunning] = useState(false);
  const [fullscreen, setFullscreen] = useState(false);
  const [deepBusy, setDeepBusy] = useState(false);
  const [deepProgress, setDeepProgress] = useState('');
  const [logs, setLogs] = useState<{ t: string; msg: string; cls: string }[]>([]);
  const frameRef = useRef<HTMLIFrameElement>(null);
  const pilotRef = useRef<any>(null);
  const timerRef = useRef<number>(0);
  const sessionRef = useRef<InspectSession | null>(null);
  const rulesRef = useRef(rules);
  const findingsRef = useRef(findings);
  const deepStopRef = useRef(false);
  rulesRef.current = rules;
  sessionRef.current = session;
  findingsRef.current = findings;

  useEffect(() => {
    api<{ config: { enabled: boolean } }>('/api/ai/config').then((d) => setAiEnabled(d.config.enabled)).catch(() => {});
    return () => {
      window.clearInterval(timerRef.current);
      pilotRef.current?.stop?.();
    };
  }, []);
  useEffect(() => { if (isLocalUrl(url)) setAuthorized(true); }, [url]);
  useEffect(() => {
    if (!fullscreen) return;
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') setFullscreen(false); };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [fullscreen]);

  const addFinding = (f: Finding, furl: string): boolean => {
    const key = findingKey(f, furl);
    if (findingsRef.current.has(key)) return false;
    const n = new Map(findingsRef.current);
    n.set(key, { ...f, url: furl });
    findingsRef.current = n;
    setFindings(n);
    return true;
  };

  const enter = async () => {
    if (!url.trim()) return toast('请填写目标网址');
    try {
      const d = await api<Omit<InspectSession, 'origin' | 'findings' | 'events'>>('/api/inspect/start', { url: url.trim(), authorized });
      document.cookie = `${d.cookieName}=${d.sid}; path=${d.cookiePath}`;
      const s: InspectSession = { ...d, origin: new URL(url.trim()).origin, findings: new Map(), events: [] };
      setSession(s);
      setFindings(new Map());
      setEvents([]);
      setStatus('巡检中：页面已加载，实时抓取控制台 / 异常 / 网络 / DOM 问题');
      window.clearInterval(timerRef.current);
      timerRef.current = window.setInterval(poll, 1500);
    } catch (e: any) {
      toast(e.message);
    }
  };

  const poll = async () => {
    const s = sessionRef.current;
    if (!s) return;
    try {
      const data = await api<{ events: CaptureEvent[] }>(`/api/captures?sid=${s.sid}`);
      setEvents(data.events);
      for (const ev of data.events) {
        if (ev.type === 'scan') {
          for (const f of (ev as any).findings || []) addFinding(f, ev.realUrl || '');
        } else if (ev.type === 'dom') {
          for (const issue of ev.issues || []) {
            const rule = rulesRef.current.find((r) => r.id === issue.ruleId);
            addFinding({
              ruleId: issue.ruleId, evidence: issue.evidence,
              title: rule?.title || issue.ruleId,
              category: rule?.category || 'ui', severity: rule?.severity || 'medium',
            }, ev.realUrl || '');
          }
        } else {
          const ruleId = ev.type === 'exception' ? 'func-js-exception'
            : ev.type === 'network' ? 'func-network-error'
            : ev.type === 'resource' ? 'func-resource-error'
            : ev.type === 'console' && ev.level === 'error' ? 'func-console-error'
            : null;
          const rule = rulesRef.current.find((r) => r.id === ruleId);
          if (rule) {
            const evidence = [ev.message, ev.source, ev.stack, ev.url].filter(Boolean).join('\n');
            addFinding({ ruleId: rule.id, title: rule.title, category: rule.category, severity: rule.severity, evidence }, ev.realUrl || '');
          }
        }
      }
    } catch { /* 轮询瞬时失败忽略 */ }
  };

  const currentRealUrl = (): string => {
    try {
      const loc = frameRef.current!.contentWindow!.location;
      const m = loc.pathname.match(/^\/proxy\/([^/]+)(\/.*)?$/);
      if (m && session) return session.origin + (m[2] || '/') + loc.search;
      // 钩子已把路径还原为干净路径（SPA 路由），直接拼目标 origin
      return (session ? session.origin : '') + loc.pathname + loc.search;
    } catch { return ''; }
  };

  const rescan = async () => {
    try {
      const doc = frameRef.current?.contentDocument;
      if (!doc?.documentElement) return toast('页面尚未加载完成');
      const furl = currentRealUrl();
      const r = await api<{ findings: Finding[] }>('/api/scan-html', { html: doc.documentElement.outerHTML, url: furl });
      r.findings.forEach((f) => addFinding(f, furl));
      toast(`当前页体检完成：${r.findings.length} 个问题`);
    } catch (e: any) { toast('体检失败：' + e.message); }
  };

  const mark = () => {
    const annotation = prompt('描述你看到的问题（界面 / 逻辑 / 功能 / 数据 / 算法）：');
    if (annotation === null) return;
    const furl = currentRealUrl();
    const errs = events
      .filter((e) => ['console', 'exception', 'network', 'resource'].includes(e.type))
      .slice(-5)
      .map((e) => `[${EV_LABEL[e.type] || e.type}] ${e.message || ''} ${e.url || ''}`)
      .join('\n');
    onFix({
      title: '人工巡检：' + annotation.slice(0, 50),
      url: furl, severity: 'medium', category: 'function',
      steps: '人工巡检操作中发现：' + annotation,
      evidence: annotation + '\n页面：' + furl + (errs ? '\n近期抓取：\n' + errs : ''),
    }, furl);
  };

  const clear = async () => {
    if (!session) return;
    await api('/api/inspect/clear', { sid: session.sid });
    setFindings(new Map());
    setEvents([]);
    toast('已清空抓取记录');
  };

  const aiLog = (msg: string, cls = '') => {
    setLogs((l) => [...l.slice(-199), { t: fmtTime(Date.now()), msg, cls }]);
  };

  const startPilot = () => {
    if (!session) return toast('请先点击「进入巡检」');
    if (pilotRef.current?.running) return;
    const pilot = new window.WfPilot(frameRef.current, {
      fx, allowSubmit, maxSteps: 24,
      getUrl: currentRealUrl,
      onLog: aiLog,
      onIssues: (issues: { ruleId: string; evidence: string }[], furl: string) => {
        let added = 0;
        for (const it of issues) {
          const rule = rulesRef.current.find((r) => r.id === it.ruleId);
          if (!rule) continue;
          const key = findingKey({ ruleId: it.ruleId, title: rule.title } as Finding, furl);
          if (!findings.has(key)) added++;
          addFinding({ ruleId: it.ruleId, evidence: it.evidence, title: rule.title, category: rule.category, severity: rule.severity }, furl);
        }
        if (added) aiLog(`识别出问题并已加入下方清单（+${added}）`, 'warn');
      },
      onDone: () => setPilotRunning(false),
    });
    pilotRef.current = pilot;
    setPilotRunning(true);
    pilot.run();
  };

  const stopPilot = () => {
    pilotRef.current?.stop?.();
    setPilotRunning(false);
    toast('已停止 AI 巡检');
  };

  const deep = async () => {
    if (!session) return toast('请先进入巡检');
    if (!aiEnabled) { toast('AI 增强未启用：请到「AI 设置」配置并启用'); return; }
    let html = '';
    try { html = frameRef.current?.contentDocument?.documentElement?.outerHTML || ''; } catch { /* 跨域忽略 */ }
    const furl = currentRealUrl();
    setAiBusy(true);
    setStatus('AI 深度挖掘中…（证据已发送给你配置的模型）');
    try {
      const d = await api<{ findings: Finding[]; model: string }>('/api/ai/analyze', {
        kind: 'inspect', url: furl,
        findings: [...findings.values()],
        console: events.filter((e) => ['console', 'exception', 'network', 'resource'].includes(e.type)),
        html,
      });
      let n = 0;
      for (const f of d.findings) {
        const key = findingKey(f, furl);
        if (!findings.has(key)) n++;
        addFinding(f, furl);
      }
      setStatus(`AI 深度挖掘完成：新增 ${n} 条（模型 ${d.model}）`);
    } catch (e: any) {
      setStatus('AI 挖掘失败：' + e.message);
    } finally {
      setAiBusy(false);
    }
  };

  const list = [...findings.values()];
  const feed = events.slice(-60).reverse();

  const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

  const waitForDoc = async (timeoutMs = 9000): Promise<Document | null> => {
    const start = Date.now();
    while (Date.now() - start < timeoutMs) {
      try {
        const doc = frameRef.current?.contentDocument;
        if (doc && doc.body && doc.readyState !== 'loading') return doc;
      } catch { /* 跨域瞬间 */ }
      await wait(300);
    }
    try { return frameRef.current?.contentDocument || null; } catch { return null; }
  };

  const proxyForRoute = (routeUrl: string): string => {
    const u = new URL(routeUrl);
    return `/proxy/${session!.token}${u.pathname}${u.search}`;
  };

  // 智能逐页深度探测：发现全站路由 → 逐页在内嵌浏览器渲染 → 渲染态 UI 识别 + DOM 体检 + 后端主动漏洞探测
  const deepScanAll = async () => {
    if (!session) return toast('请先点击「进入巡检」');
    if (deepBusy) return;
    setDeepBusy(true);
    deepStopRef.current = false;
    pilotRef.current?.stop?.();
    setPilotRunning(false);
    let pages = 0;
    let added = 0;
    try {
      setStatus('正在智能发现全站路由（链接 + robots + sitemap + 前端路由）…');
      aiLog('开始逐页深度探测：智能发现全站路由', 'act');
      const { routes } = await api<{ routes: string[] }>('/api/routes', { url: url.trim(), authorized });
      if (!routes.length) { setStatus('未发现可遍历的路由'); aiLog('未发现路由', 'warn'); return; }
      aiLog(`发现 ${routes.length} 个路由，开始逐页渲染探测`, 'ok');
      for (let i = 0; i < routes.length; i++) {
        if (deepStopRef.current) { aiLog('已手动停止深度探测', 'warn'); break; }
        const route = routes[i];
        pages++;
        setDeepProgress(`逐页深度探测 ${i + 1}/${routes.length}`);
        setStatus(`逐页深度探测 ${i + 1}/${routes.length}：${route}`);
        aiLog(`[${i + 1}/${routes.length}] 渲染并探测：${route}`, 'act');
        try { frameRef.current!.src = proxyForRoute(route); } catch { /* 忽略 */ }
        await waitForDoc();
        await wait(1300); // 等待 SPA 渲染完成
        if (deepStopRef.current) break;

        // 1) 渲染态 UI 识别（点击目标 / 对比度 / 空按钮 / 图片 alt / 坏图）
        try {
          const doc = frameRef.current?.contentDocument;
          if (doc && window.WfPilot) {
            const p = new window.WfPilot(frameRef.current, { fx: false, getUrl: () => route });
            const res = p.perceive(doc);
            for (const it of res.issues || []) {
              const rule = rulesRef.current.find((r) => r.id === it.ruleId);
              if (!rule) continue;
              if (addFinding({ ruleId: it.ruleId, evidence: it.evidence, title: rule.title, category: rule.category, severity: rule.severity }, route)) added++;
            }
          }
        } catch { /* 忽略 */ }

        // 2) 渲染态 DOM 体检（覆盖动态渲染内容）
        try {
          const doc = frameRef.current?.contentDocument;
          if (doc?.documentElement) {
            const r = await api<{ findings: Finding[] }>('/api/scan-html', { html: doc.documentElement.outerHTML, url: route });
            for (const f of r.findings) if (addFinding(f, route)) added++;
          }
        } catch { /* 忽略 */ }

        // 3) 后端主动漏洞探测 + 响应头 + 链接/资源健康（真实目标地址）
        try {
          const r = await api<{ findings: Finding[] }>('/api/scan', { url: route, authorized, active: true, assets: true });
          for (const f of r.findings) if (addFinding(f, route)) added++;
        } catch { /* 忽略 */ }

        aiLog(`  ↳ 完成（累计新增 ${added} 条）`, 'ok');
      }
      setDeepProgress('');
      setStatus(`逐页深度探测完成：遍历 ${pages} 个页面，新增 ${added} 条，累计 ${findingsRef.current.size} 个问题`);
      aiLog(`逐页深度探测结束：遍历 ${pages} 页，新增 ${added} 条`, 'ok');
      toast(`逐页深度探测完成：${pages} 个页面，新增 ${added} 条问题`);
    } catch (e: any) {
      setDeepProgress('');
      setStatus('逐页深度探测失败：' + e.message);
      aiLog('深度探测异常：' + e.message, 'warn');
    } finally {
      setDeepBusy(false);
    }
  };

  const stopDeep = () => { deepStopRef.current = true; toast('正在停止深度探测…'); };

  return (
    <div className="p-6 max-w-[1200px] mx-auto">
      <PageHeader id="inspect" />
      <Section
        title="人工巡检"
        desc="内嵌浏览器实时抓取控制台 / 异常 / 网络 / DOM 问题；支持登录墙后台巡检与 AI 自动操作识别。"
        actions={<button className="btn !h-8 !text-xs" onClick={enter}>进入巡检</button>}
      >
        <label className="block">
          <span className="label">目标网址（支持登录墙：在下方内嵌浏览器中登录即可巡检后台）</span>
          <input className="input" type="url" value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://你的网站.example" />
        </label>
        <label className="flex items-center gap-2 mt-3 text-[13px] text-ink-2 cursor-pointer">
          <input type="checkbox" className="accent-[#1677ff]" checked={authorized} onChange={(e) => setAuthorized(e.target.checked)} />
          我拥有该网站或已获得测试授权（本机演示地址无需勾选）
        </label>
        {session && (
          <div className="flex flex-wrap items-center gap-2 mt-4">
            <button className="btn-ghost !h-8 !text-xs" onClick={rescan}>体检当前页面</button>
            <button className="btn-ghost !h-8 !text-xs" onClick={mark}>标记当前页面 BUG</button>
            <button className="btn-ghost !h-8 !text-xs" onClick={clear}>清空抓取记录</button>
            <button className="btn-ghost !h-8 !text-xs" disabled={aiBusy} onClick={deep}>{aiBusy ? 'AI 挖掘中…' : 'AI 深度挖掘当前页'}</button>
            <span className="w-px h-5 bg-line mx-1" />
            <button className="btn !h-8 !text-xs" disabled={deepBusy} onClick={deepScanAll}>{deepBusy ? '逐页探测中…' : '智能逐页深度探测'}</button>
            <button className="btn-ghost !h-8 !text-xs" disabled={!deepBusy} onClick={stopDeep}>停止探测</button>
            <span className="w-px h-5 bg-line mx-1" />
            <button className="btn !h-8 !text-xs" disabled={pilotRunning || deepBusy} onClick={startPilot}>AI 自动巡检</button>
            <button className="btn-ghost !h-8 !text-xs" disabled={!pilotRunning} onClick={stopPilot}>停止 AI</button>
            <label className="flex items-center gap-1.5 text-xs text-ink-2 cursor-pointer ml-1">
              <input type="checkbox" className="accent-[#1677ff]" checked={fx} onChange={(e) => setFx(e.target.checked)} />识别特效
            </label>
            <label className="flex items-center gap-1.5 text-xs text-ink-2 cursor-pointer">
              <input type="checkbox" className="accent-[#1677ff]" checked={allowSubmit} onChange={(e) => setAllowSubmit(e.target.checked)} />允许提交类按钮
            </label>
            <span className="w-px h-5 bg-line mx-1" />
            <button className="btn-ghost !h-8 !text-xs" onClick={() => setFullscreen(true)}>全屏展示</button>
            <button className="btn-ghost !h-8 !text-xs" onClick={() => window.open(session.proxyUrl, '_blank', 'noopener')}>新窗口打开</button>
          </div>
        )}
        {status && <p className="text-[13px] text-ink-2 mt-2.5 animate-paneIn">{status}</p>}
        {session && <p className="text-xs text-ink-3 mt-2">巡检会话 ID（供协同编排使用）：<code className="select-all">{session.sid}</code></p>}
        {deepBusy && deepProgress && (
          <div className="mt-2 animate-paneIn">
            <div className="meter"><i className="bg-accent transition-all duration-500" style={{ width: '100%' }} /></div>
            <p className="text-xs text-ink-3 mt-1.5">{deepProgress} · 渲染态识别 + DOM 体检 + 主动漏洞探测</p>
          </div>
        )}
      </Section>

      {logs.length > 0 && (
        <div className="card p-3 mb-4 max-h-44 overflow-auto font-mono text-[11px] leading-relaxed animate-paneIn">
          {logs.map((l, i) => (
            <p key={i} className={l.cls === 'warn' ? 'text-warn' : l.cls === 'ok' ? 'text-ok' : l.cls === 'act' ? 'text-accent' : 'text-ink-2'}>
              [{l.t}] {l.msg}
            </p>
          ))}
        </div>
      )}

      {session && (
        <div
          className={fullscreen ? 'fixed inset-0 z-[200] bg-white flex flex-col animate-paneIn' : 'card overflow-hidden mb-4 animate-paneIn flex flex-col'}
          style={fullscreen ? undefined : { height: 620 }}
        >
          {fullscreen && (
            <div className="flex items-center justify-between gap-3 px-4 h-12 border-b border-line bg-panel shrink-0">
              <span className="text-[13px] text-ink-2 truncate">巡检浏览器（全屏）· {url}</span>
              <div className="flex items-center gap-2 shrink-0">
                <button className="btn-ghost !h-8 !text-xs" onClick={rescan}>体检当前页</button>
                <button className="btn-ghost !h-8 !text-xs" onClick={() => window.open(session.proxyUrl, '_blank', 'noopener')}>新窗口打开</button>
                <button className="btn !h-8 !text-xs" onClick={() => setFullscreen(false)}>退出全屏 (Esc)</button>
              </div>
            </div>
          )}
          <iframe
            ref={frameRef}
            title="巡检浏览器"
            src={session.proxyUrl}
            className="w-full flex-1 border-0 bg-white"
            allow="clipboard-write; fullscreen; camera; microphone"
          />
        </div>
      )}

      {session && (
        <Section title={`实时抓取 · ${events.length}`} bodyClass="px-5 py-3">
          <div className="max-h-56 overflow-auto space-y-1.5">
            {feed.length === 0 && <p className="text-xs text-ink-3">暂无抓取事件，操作页面后自动记录…</p>}
            {feed.map((ev, i) => (
              <div key={i} className="flex items-start gap-2 text-xs">
                <span className={`badge shrink-0 ${ev.type === 'exception' || ev.type === 'network' ? 'high' : ev.type === 'console' ? 'medium' : 'cat'}`}>
                  {EV_LABEL[ev.type] || ev.type}
                </span>
                <div className="min-w-0">
                  <span className="text-ink-2 break-all">{ev.message || ev.title || ''}</span>
                  {ev.url && <span className="text-ink-3 ml-2 break-all">{ev.realUrl || ev.url}</span>}
                  <span className="text-ink-3 ml-2">{fmtTime(ev.ts)}</span>
                </div>
              </div>
            ))}
          </div>
        </Section>
      )}

      {list.length > 0 && (
        <>
          <SummaryBar summary={summaryOf(list)} label="巡检累计发现" />
          <FindingFilters value={filter} onChange={setFilter} total={list.length} count={list.filter(f => matchesFinding(f, filter)).length} />
          {list.filter(f => matchesFinding(f, filter)).map((f, i) => (
            <FindingCard key={findingKey(f, f.url || '')} f={f} index={i} onFix={(x) => onFix(x, x.url)} />
          ))}
        </>
      )}
    </div>
  );
}