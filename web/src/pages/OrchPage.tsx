import { useEffect, useRef, useState } from 'react';
import { api, isLocalUrl } from '../lib/api';
import type { Extension, Finding, Pipeline, PipelineRun, PipelineStep } from '../lib/types';
import { useToast } from '../components/Toast';
import { Confirm, Modal } from '../components/Modal';
import { PageHeader } from '../components/PageHeader';
import { SummaryBar } from '../components/SummaryBar';
import { FindingCard } from '../components/FindingCard';
import { Ico, Section, StatCard } from '../components/ui';

const MODE_LABEL: Record<string, string> = { serial: '串行', parallel: '并行', conditional: '条件' };
const OPS = ['truthy', 'falsy', 'eq', 'ne', 'gt', 'gte', 'lt', 'lte', 'includes'];

export function OrchPage({ onFix }: { onFix: (f: Finding, url?: string) => void }) {
  const toast = useToast();
  const [pipelines, setPipelines] = useState<Pipeline[]>([]);
  const [exts, setExts] = useState<Extension[]>([]);
  const [pipeId, setPipeId] = useState('');
  const [url, setUrl] = useState(location.origin + '/demo');
  const [auth, setAuth] = useState(false);
  const [running, setRunning] = useState(false);
  const [run, setRun] = useState<PipelineRun | null>(null);
  const [editor, setEditor] = useState<Pipeline | null | undefined>(undefined); // undefined=closed, null=new
  const [delPipe, setDelPipe] = useState(false);
  const timer = useRef<ReturnType<typeof setInterval> | null>(null);
  const pipeIdRef = useRef(pipeId);
  pipeIdRef.current = pipeId;

  const loadExts = async () => {
    try {
      const d = await api<{ extensions: Extension[] }>('/api/ext/list');
      setExts(d.extensions);
    } catch (e: any) { toast('扩展加载失败：' + e.message); }
  };
  const loadPipelines = async (keepId?: string) => {
    try {
      const d = await api<{ pipelines: Pipeline[] }>('/api/orch/pipelines');
      setPipelines(d.pipelines);
      const want = keepId || pipeIdRef.current;
      const id = d.pipelines.some((p) => p.id === want) ? want : (d.pipelines[0]?.id || '');
      setPipeId(id);
    } catch (e: any) { toast('编排加载失败：' + e.message); }
  };
  useEffect(() => {
    loadPipelines();
    loadExts();
    return () => { if (timer.current) clearInterval(timer.current); };
  }, []);

  const pipe = pipelines.find((p) => p.id === pipeId) || null;

  const pollRun = async (id: string) => {
    try {
      const r = await api<PipelineRun>('/api/orch/run-status?id=' + encodeURIComponent(id));
      setRun(r);
      if (r.status !== 'running') {
        if (timer.current) { clearInterval(timer.current); timer.current = null; }
        setRunning(false);
      }
    } catch (e: any) {
      if (timer.current) { clearInterval(timer.current); timer.current = null; }
      setRunning(false);
      toast('运行状态查询失败：' + e.message);
    }
  };

  const startRun = async () => {
    if (!pipe) { toast('请先选择或新建编排'); return; }
    const u = url.trim();
    if (!u) { toast('请先填写目标网址'); return; }
    if (!isLocalUrl(u) && !auth) { toast('请先勾选授权确认'); return; }
    setRunning(true);
    setRun(null);
    try {
      const d = await api<{ id: string }>('/api/orch/run', { pipelineId: pipe.id, url: u, authorized: isLocalUrl(u) || auth });
      if (timer.current) clearInterval(timer.current);
      timer.current = setInterval(() => pollRun(d.id), 700);
      pollRun(d.id);
    } catch (e: any) { toast(e.message); setRunning(false); }
  };

  const removePipe = async () => {
    if (!pipe) return;
    try {
      await api('/api/orch/pipeline/remove', { id: pipe.id });
      toast('已删除');
      setPipeId('');
      loadPipelines();
    } catch (e: any) { toast(e.message); }
  };

  const report = run && run.status !== 'running'
    ? Object.values(run.outputs || {}).find((o): o is { report: string } => !!o && typeof (o as any).report === 'string')
    : undefined;
  const doneKeys = new Set((run?.steps || []).map((s) => s.key));
  const pending = run?.status === 'running' && pipe ? pipe.steps.filter((s) => !doneKeys.has(s.key)) : [];

  return (
    <div className="p-6 max-w-[1060px] mx-auto">
      <PageHeader id="orch" />
      <div className="grid grid-cols-3 gap-3 mb-4">
        <StatCard color="blue" label="编排流程" value={pipelines.length} sub="串行 / 并行 / 条件"
          icon={<Ico d="M4 6h16M4 12h10M4 18h7m5-3 4 3-4 3" />} />
        <StatCard color="green" label="启用扩展" value={exts.filter((e) => e.enabled).length} sub={`共 ${exts.length} 个已注册`}
          icon={<Ico d="M12 2l3 3-3 3-3-3Zm-7 7l3 3-3 3-3-3Zm14 0l3 3-3 3-3-3Zm-7 7l3 3-3 3-3-3Z" />} />
        <StatCard color="purple" label="当前模式" value={pipe ? (MODE_LABEL[pipe.mode] || pipe.mode) : '—'} sub={pipe ? pipe.name : '未选择编排'}
          icon={<Ico d="M4 7h16M4 12h16M4 17h10" />} />
      </div>
      <Section
        title="协同编排"
        desc="串行 / 并行 / 条件流程编排智能体与扩展，上下文自动传递（url / findings / stats / outputs），失败可重试。"
        actions={<button className="btn !h-8 !text-xs" disabled={running} onClick={startRun}>{running ? '运行中…' : '运行编排'}</button>}
      >
        <div className="flex flex-wrap items-end gap-3">
          <label className="flex-1 min-w-[260px]">
            <span className="label">目标网址</span>
            <input className="input" value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://你的网站.example" />
          </label>
          <label className="flex items-center gap-2 text-[13px] text-ink-2 cursor-pointer h-9">
            <input type="checkbox" className="accent-[#1677ff]" checked={auth} onChange={(e) => setAuth(e.target.checked)} />
            我已获得该网站测试授权
          </label>
        </div>
        <div className="flex flex-wrap items-center gap-2 mt-4">
          <select className="input !w-auto min-w-[240px]" value={pipeId} onChange={(e) => setPipeId(e.target.value)}>
            {pipelines.map((p) => <option key={p.id} value={p.id}>{p.name}（{MODE_LABEL[p.mode] || p.mode}）</option>)}
          </select>
          <button className="btn-ghost" onClick={() => setEditor(null)}>新建编排</button>
          <button className="btn-ghost" onClick={() => { if (!pipe) { toast('请先选择编排'); return; } setEditor(pipe); }}>编辑</button>
          <button className="btn-danger" onClick={() => setDelPipe(true)}>删除</button>
        </div>
        {pipe ? (
          <div className="mt-4">
            <p className="text-xs text-ink-3 mb-2">
              {pipe.description}
              <span className="badge cat ml-2">{MODE_LABEL[pipe.mode] || pipe.mode}</span>
            </p>
            <div className="flex flex-wrap items-center gap-2">
              {pipe.steps.map((s, i) => (
                <span key={i} className="flex items-center gap-2">
                  {i > 0 && <span className="text-ink-3">{pipe.mode === 'parallel' ? '⇄' : '→'}</span>}
                  <span className="card !rounded-lg px-3 py-1.5 flex flex-col items-start">
                    <span className="text-xs font-semibold text-ink">{s.key}</span>
                    <span className="text-[11px] text-ink-3">{s.ext}</span>
                    {s.when && <span className="text-[11px] text-accent">当 {s.when.path} {s.when.op} {String(s.when.value ?? '')}</span>}
                  </span>
                </span>
              ))}
            </div>
          </div>
        ) : (
          <p className="text-xs text-ink-3 mt-4">暂无编排，点击「新建编排」。</p>
        )}
      </Section>

      {run && (
        <Section
          title={`运行 · ${run.name}`}
          actions={<span className={`text-xs ${run.status === 'running' ? 'text-accent animate-blink' : run.status === 'done' ? 'text-ok' : 'text-danger'}`}>
            {run.status === 'running' ? '运行中…' : run.status === 'done' ? `完成 · ${run.findings.length} 条发现` : '失败：' + (run.error || '')}
          </span>}
        >
          <div className="space-y-1.5 mb-3">
            {run.steps.map((s, i) => (
              <div key={i} className={`flex items-center gap-3 text-xs rounded-lg px-3 py-2 border ${
                s.status === 'ok' ? 'border-line bg-side' : s.status === 'error' ? 'border-danger/30 bg-danger/5' : s.status === 'skipped' ? 'border-line bg-side opacity-70' : 'border-accent/30 bg-accent-soft'
              }`}>
                <span className="font-semibold text-ink w-24 truncate">{s.key}</span>
                <span className="text-ink-3 w-32 truncate">{s.ext}</span>
                {s.status === 'skipped' && <span className="text-ink-3">条件不满足，跳过</span>}
                {s.error && <span className="text-danger truncate">{s.error}</span>}
                <span className="ml-auto text-ink-3 tabular-nums shrink-0">{s.findings ? s.findings + ' 条 · ' : ''}{s.ms}ms</span>
              </div>
            ))}
            {pending.map((s, i) => (
              <div key={'p' + i} className={`flex items-center gap-3 text-xs rounded-lg px-3 py-2 border ${i === 0 ? 'border-accent/30 bg-accent-soft' : 'border-line bg-side opacity-60'}`}>
                <span className="font-semibold text-ink w-24 truncate">{s.key}</span>
                <span className="text-ink-3 w-32 truncate">{s.ext}</span>
                <span className="ml-auto text-ink-3">{i === 0 ? '执行中' : '等待'}</span>
              </div>
            ))}
          </div>
          {run.summary && run.status !== 'running' && (
            <SummaryBar summary={run.summary} label={`协同编排「${run.name}」· ${MODE_LABEL[run.mode] || run.mode} · 用时 ${Math.round(((run.finishedAt || 0) - run.startedAt) / 1000)}s`} />
          )}
          {report && (
            <div className="mb-3">
              <div className="text-xs font-medium text-ink-2 mb-1.5">Bot 汇总报告</div>
              <pre className="code !max-h-72 whitespace-pre-wrap">{report.report}</pre>
            </div>
          )}
          {run.status !== 'running' && (
            run.findings.length
              ? run.findings.map((f, i) => <FindingCard key={i} f={f} index={i} onFix={(x) => onFix(x, url)} />)
              : <div className="card p-4 text-xs text-ink-3">未发现问题</div>
          )}
        </Section>
      )}

      {editor !== undefined && (
        <PipeEditor
          pipe={editor}
          exts={exts.filter((e) => e.enabled)}
          onClose={() => setEditor(undefined)}
          onSaved={(id) => { setEditor(undefined); loadPipelines(id); }}
        />
      )}
      {delPipe && pipe && (
        <Confirm
          title="删除编排" danger okText="删除"
          message={`删除编排「${pipe.name}」？此操作不可恢复。`}
          onOk={removePipe} onClose={() => setDelPipe(false)}
        />
      )}
    </div>
  );
}

interface EditStep { key: string; ext: string; wpath: string; wop: string; wval: string; onError: string; retry: string; timeoutMs: string }

function toEdit(s: PipelineStep): EditStep {
  return {
    key: s.key || '', ext: s.ext, wpath: s.when?.path || '', wop: s.when?.op || '',
    wval: s.when?.value != null ? String(s.when.value) : '',
    onError: s.onError === 'stop' ? 'stop' : 'continue',
    retry: String(Number(s.retry) || 0), timeoutMs: String(Number(s.timeoutMs) || 0),
  };
}

function PipeEditor({ pipe, exts, onClose, onSaved }: { pipe: Pipeline | null; exts: Extension[]; onClose: () => void; onSaved: (id: string) => void }) {
  const toast = useToast();
  const [name, setName] = useState(pipe?.name || '');
  const [mode, setMode] = useState(pipe?.mode || 'serial');
  const [description, setDescription] = useState(pipe?.description || '');
  const [steps, setSteps] = useState<EditStep[]>(
    (pipe?.steps || [{ key: 'scan', ext: 'agent-rule' } as PipelineStep]).map(toEdit),
  );
  const [saving, setSaving] = useState(false);

  if (!exts.length) {
    return (
      <Modal title="新建编排" onClose={onClose}>
        <p className="text-[13px] text-ink-2">请先在扩展中心启用至少一个扩展。</p>
      </Modal>
    );
  }

  const patch = (i: number, p: Partial<EditStep>) => setSteps((s) => s.map((x, j) => (j === i ? { ...x, ...p } : x)));

  const save = async () => {
    if (!steps.length) { toast('至少一个步骤'); return; }
    const bodySteps = steps.map((s) => {
      const out: Record<string, unknown> = {
        key: s.key.trim(), ext: s.ext, onError: s.onError,
        retry: Number(s.retry) || 0, timeoutMs: Number(s.timeoutMs) || 0,
      };
      if (s.wop) {
        const num = Number(s.wval);
        out.when = { path: s.wpath.trim(), op: s.wop, value: s.wval !== '' && Number.isFinite(num) ? num : s.wval };
      }
      return out;
    });
    setSaving(true);
    try {
      const r = await api<{ pipeline: Pipeline }>('/api/orch/pipeline', {
        id: pipe?.id, name: name.trim() || '未命名编排', mode, description: description.trim(), steps: bodySteps,
      });
      toast('编排已保存');
      onSaved(r.pipeline.id);
    } catch (e: any) { toast(e.message); }
    setSaving(false);
  };

  return (
    <Modal title={pipe ? '编辑编排' : '新建编排'} onClose={onClose} wide>
      <div className="grid grid-cols-2 gap-3">
        <label><span className="label">名称</span>
          <input className="input" value={name} onChange={(e) => setName(e.target.value)} />
        </label>
        <label><span className="label">流程模式</span>
          <select className="input" value={mode} onChange={(e) => setMode(e.target.value)}>
            {Object.entries(MODE_LABEL).map(([k, v]) => <option key={k} value={k}>{v} {k}</option>)}
          </select>
        </label>
      </div>
      <label className="block mt-3"><span className="label">描述</span>
        <input className="input" value={description} onChange={(e) => setDescription(e.target.value)} />
      </label>
      <div className="space-y-3 mt-4">
        {steps.map((s, i) => (
          <div key={i} className="border border-line rounded-pane p-3 bg-side/60">
            <div className="flex items-center justify-between mb-2">
              <span className="text-xs font-semibold text-ink">步骤 {i + 1}</span>
              <button className="btn-danger !h-6 !px-2 !text-[11px]" onClick={() => setSteps((x) => x.filter((_, j) => j !== i))}>移除</button>
            </div>
            <div className="grid grid-cols-2 gap-2.5">
              <label><span className="label">Key</span>
                <input className="input" value={s.key} onChange={(e) => patch(i, { key: e.target.value })} placeholder="scan" />
              </label>
              <label><span className="label">扩展</span>
                <select className="input" value={s.ext} onChange={(e) => patch(i, { ext: e.target.value })}>
                  {exts.map((e) => <option key={e.id} value={e.id}>{e.name}（{e.id}）</option>)}
                </select>
              </label>
              <label><span className="label">条件路径（可空）</span>
                <input className="input" value={s.wpath} onChange={(e) => patch(i, { wpath: e.target.value })} placeholder="stats.critical / aiEnabled" />
              </label>
              <label><span className="label">条件操作</span>
                <select className="input" value={s.wop} onChange={(e) => patch(i, { wop: e.target.value })}>
                  <option value="">无条件</option>
                  {OPS.map((o) => <option key={o} value={o}>{o}</option>)}
                </select>
              </label>
              <label><span className="label">条件值</span>
                <input className="input" value={s.wval} onChange={(e) => patch(i, { wval: e.target.value })} placeholder="1" />
              </label>
              <label><span className="label">失败策略</span>
                <select className="input" value={s.onError} onChange={(e) => patch(i, { onError: e.target.value })}>
                  <option value="continue">continue 继续</option>
                  <option value="stop">stop 中断</option>
                </select>
              </label>
              <label><span className="label">重试次数</span>
                <input className="input" type="number" min={0} max={3} value={s.retry} onChange={(e) => patch(i, { retry: e.target.value })} />
              </label>
              <label><span className="label">超时 ms（0=默认）</span>
                <input className="input" type="number" min={0} value={s.timeoutMs} onChange={(e) => patch(i, { timeoutMs: e.target.value })} />
              </label>
            </div>
          </div>
        ))}
      </div>
      <div className="flex gap-2 mt-4">
        <button className="btn-ghost" onClick={() => setSteps((x) => [...x, toEdit({ key: '', ext: exts[0].id } as PipelineStep)])}>+ 添加步骤</button>
        <button className="btn" disabled={saving} onClick={save}>保存编排</button>
        <button className="btn-ghost" onClick={onClose}>取消</button>
      </div>
      <p className="text-[11px] text-ink-3 mt-3 leading-relaxed">
        上下文自动传递：url / findings / stats / outputs / headers / html。条件示例：path=aiEnabled op=truthy（仅启用 AI 时执行）；path=stats.critical op=gte value=1（存在严重问题时追加挖掘）。
      </p>
    </Modal>
  );
}