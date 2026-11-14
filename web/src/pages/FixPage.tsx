import { useEffect, useState } from 'react';
import { api, copyText, download } from '../lib/api';
import { CAT, SEV } from '../lib/format';
import type { Finding, FixResult, Rule } from '../lib/types';
import type { FixRequest } from '../App';
import { useToast } from '../components/Toast';
import { PageHeader } from '../components/PageHeader';
import { Section } from '../components/ui';

const EMPTY = {
  title: '', url: '', severity: 'medium', category: 'security',
  steps: '', evidence: '', rootCause: '',
};

export function FixPage({ rules, request }: { rules: Rule[]; request: FixRequest | null }) {
  const toast = useToast();
  const [f, setF] = useState({ ...EMPTY });
  const [frontend, setFrontend] = useState('原生 HTML/JS');
  const [backend, setBackend] = useState('Node.js (Express)');
  const [template, setTemplate] = useState('');
  const [brief, setBrief] = useState(true);
  const [result, setResult] = useState<FixResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [ruleId, setRuleId] = useState<string | null>(null);
  const [extra, setExtra] = useState<{ fix?: any; verify?: string[]; guard?: string } | null>(null);
  const [lastSeq, setLastSeq] = useState(0);

  useEffect(() => {
    if (!request || request.seq === lastSeq) return;
    setLastSeq(request.seq);
    const x = request.finding;
    const rule = rules.find((r) => r.id === x.ruleId);
    setRuleId(x.ruleId || null);
    setExtra({ fix: x.fix, verify: x.verify, guard: x.guard });
    setTemplate(x.ruleId || '');
    setF({
      title: x.title || '',
      url: request.url || x.url || '',
      severity: x.severity || 'medium',
      category: x.category || 'security',
      steps: x.steps || ('扫描/巡检自动发现；' + String(x.evidence || '').split('\n')[0]),
      evidence: x.evidence || '',
      rootCause: x.rootCause || rule?.rootCause || '',
    });
    setResult(null);
    // 自动载入即生成
    generateWith({
      title: x.title || '', url: request.url || x.url || '', severity: x.severity || 'medium',
      category: x.category || 'security',
      steps: x.steps || ('扫描/巡检自动发现；' + String(x.evidence || '').split('\n')[0]),
      evidence: x.evidence || '', rootCause: x.rootCause || rule?.rootCause || '',
    }, x.ruleId || null, { fix: x.fix, verify: x.verify, guard: x.guard });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [request]);

  const set = (k: keyof typeof EMPTY) => (e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement>) =>
    setF((s) => ({ ...s, [k]: e.target.value }));

  const pickTemplate = (id: string) => {
    setTemplate(id);
    setRuleId(id || null);
    setExtra(null);
    if (!id) return;
    const r = rules.find((x) => x.id === id);
    if (!r) return;
    setF((s) => ({
      ...s, title: r.title, severity: r.severity, category: r.category, rootCause: r.rootCause,
      steps: s.steps || '（请填写实际复现步骤）',
      evidence: s.evidence || '（请粘贴截图说明 / 请求响应 / 日志证据）',
    }));
  };

  const collect = (fields = f, rid = ruleId, ex = extra) => ({
    ruleId: rid || template || null,
    ...(ex || {}),
    ...fields,
  });

  const generateWith = async (fields: typeof EMPTY, rid: string | null, ex: typeof extra) => {
    if (!fields.title.trim()) return toast('请先填写 BUG 标题，或从体检 / 爬行 / 巡检 / 模板进入');
    setBusy(true);
    try {
      const r = await api<FixResult>('/api/fix', {
        finding: { ruleId: rid || template || null, ...(ex || {}), ...fields },
        stack: { frontend, backend },
      });
      setResult(r);
    } catch (e: any) {
      toast(e.message);
    } finally {
      setBusy(false);
    }
  };

  const generate = () => generateWith(f, ruleId, extra);

  const copyPrompt = async () => {
    try {
      const r = await api<{ prompt: string }>('/api/prompt', {
        fields: collect(), stack: { frontend, backend }, style: brief ? 'brief' : 'full',
      });
      (await copyText(r.prompt)) ? toast('修复提示词已复制') : toast('复制失败，请手动选择文本');
    } catch (e: any) { toast(e.message); }
  };

  const downloadPatch = async () => {
    if (!result) return toast('请先生成四件套修复方案');
    const text = await api<string>('/api/patch', {
      items: [{ title: result.fields.title, severity: result.fields.severity, gitDiff: result.deliverables.gitDiff }],
    });
    download('fix.patch', text);
    toast('已下载 fix.patch（路径需按实际项目调整）');
  };

  const downloadReport = async () => {
    if (!result) return toast('请先生成四件套修复方案');
    const text = await api<string>('/api/report', { finding: collect(), stack: { frontend, backend } });
    download('bugfix-report.md', text, 'text/markdown;charset=utf-8');
    toast('已下载报告');
  };

  const groups = { manual: rules.filter((r) => r.manual), auto: rules.filter((r) => !r.manual) };
  const opt = (r: Rule) => <option key={r.id} value={r.id}>[{CAT[r.category] || r.category}] {r.title}</option>;
  const d = result?.deliverables;

  const CodePanel = ({ title, content }: { title: string; content: string }) => (
    <Section title={title} bodyClass="p-4"
      actions={<button className="btn-ghost !h-7 !px-2.5 !text-xs" onClick={async () => { await copyText(content); toast('已复制'); }}>复制</button>}>
      <pre className="code !max-h-96">{content}</pre>
    </Section>
  );

  return (
    <div className="p-6 max-w-[1080px] mx-auto">
      <PageHeader id="fix" />
      <Section
        title="BUG / 漏洞信息（七字段）"
        desc="填写或从体检 / 爬行 / 巡检 / 模板载入，自动生成四件套修复方案与精简提示词。"
        actions={<button className="btn !h-8 !text-xs" disabled={busy} onClick={generate}>{busy ? '生成中…' : '生成四件套修复方案'}</button>}
      >
        <div className="grid grid-cols-2 gap-3">
          <label><span className="label">BUG 标题</span><input className="input" value={f.title} onChange={set('title')} /></label>
          <label><span className="label">页面 URL</span><input className="input" value={f.url} onChange={set('url')} /></label>
          <label>
            <span className="label">风险等级</span>
            <select className="input" value={f.severity} onChange={set('severity')}>
              {Object.entries(SEV).map(([k, v]) => <option key={k} value={k}>{v} {k}</option>)}
            </select>
          </label>
          <label>
            <span className="label">问题类别</span>
            <select className="input" value={f.category} onChange={set('category')}>
              {Object.entries(CAT).map(([k, v]) => <option key={k} value={k}>{v}</option>)}
            </select>
          </label>
        </div>
        <label className="block mt-3"><span className="label">复现步骤</span><textarea className="textarea" rows={2} value={f.steps} onChange={set('steps')} /></label>
        <label className="block mt-3"><span className="label">截图 / 证据</span><textarea className="textarea" rows={2} value={f.evidence} onChange={set('evidence')} /></label>
        <label className="block mt-3"><span className="label">根因分析</span><textarea className="textarea" rows={2} value={f.rootCause} onChange={set('rootCause')} /></label>
        <div className="grid grid-cols-2 gap-3 mt-3">
          <label><span className="label">前端技术栈</span><input className="input" value={frontend} onChange={(e) => setFrontend(e.target.value)} /></label>
          <label><span className="label">后端技术栈</span><input className="input" value={backend} onChange={(e) => setBackend(e.target.value)} /></label>
        </div>
        <label className="block mt-3">
          <span className="label">或选择常见 BUG 模板（自动填充七字段）</span>
          <select className="input" value={template} onChange={(e) => pickTemplate(e.target.value)}>
            <option value="">— 手动填写 —</option>
            <optgroup label="挖掘模板（逻辑/数据/算法/安全深层等）">{groups.manual.map(opt)}</optgroup>
            <optgroup label="自动规则（体检/爬行/巡检）">{groups.auto.map(opt)}</optgroup>
          </select>
        </label>
        <label className="flex items-center gap-2 mt-3 text-[13px] text-ink-2 cursor-pointer">
          <input type="checkbox" className="accent-[#1677ff]" checked={brief} onChange={(e) => setBrief(e.target.checked)} />
          精简提示词（重点：问题描述 + 如何修复，不啰嗦）
        </label>
        <div className="flex flex-wrap gap-2 mt-4">
          <button className="btn-ghost" onClick={copyPrompt}>复制修复提示词</button>
          <button className="btn-ghost" onClick={downloadPatch}>下载 .patch 补丁</button>
          <button className="btn-ghost" onClick={downloadReport}>下载报告 .md</button>
        </div>
      </Section>

      {d && result && (
        <div className="animate-paneIn">
          <CodePanel title="1. 最简修复方案 · 前端" content={d.minimalFix.frontend} />
          <CodePanel title="1. 最简修复方案 · 后端" content={d.minimalFix.backend} />
          <CodePanel title="2. 修复后验证步骤" content={d.verifySteps.map((s, i) => `${i + 1}. ${s}`).join('\n')} />
          <CodePanel title="3. 防止同类 BUG 的防护代码" content={d.guardCode} />
          <CodePanel title="4. 可直接提交的 git diff（建议补丁）" content={d.gitDiff} />
          <Section
            title={`修复提示词（${brief ? '精简：问题 + 修复' : '详细七字段'}，可粘贴给任意 AI 助手）`}
            bodyClass="p-4"
            actions={<button className="btn-ghost !h-7 !px-2.5 !text-xs" onClick={async () => { await copyText(brief ? result.prompt : result.fullPrompt); toast('已复制'); }}>复制提示词</button>}
          >
            <pre className="code !max-h-96">{brief ? result.prompt : result.fullPrompt}</pre>
          </Section>
        </div>
      )}
    </div>
  );
}