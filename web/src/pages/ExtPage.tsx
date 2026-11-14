import { useEffect, useState, type ReactNode } from 'react';
import { api, isLocalUrl } from '../lib/api';
import type { Extension } from '../lib/types';
import { useToast } from '../components/Toast';
import { Confirm, Modal } from '../components/Modal';
import { PageHeader } from '../components/PageHeader';
import { IdBadge, Section, StatCard, Switch, Tag, VerPill, type CatColor } from '../components/ui';

interface ExtMeta { types: string[]; transports: string[]; permissions: string[]; skillsDir: string }
interface AuditEntry { ts?: string; ok?: boolean; action: string; extId?: string; pipelineId?: string; ms?: number; error?: string }

const EXT_TYPE: Record<string, { label: string; desc: string; icon: ReactNode }> = {
  agent: {
    label: '智能体', desc: '执行扫描 / 挖掘任务',
    icon: <svg viewBox="0 0 24 24" className="w-5 h-5" fill="none" stroke="currentColor" strokeWidth="1.6"><circle cx="12" cy="8" r="4" /><path d="M4 20c0-4 3.6-6.5 8-6.5s8 2.5 8 6.5" /></svg>,
  },
  skill: {
    label: 'SKILL 技能', desc: '可复用检查 / 挖掘模板',
    icon: <svg viewBox="0 0 24 24" className="w-5 h-5" fill="none" stroke="currentColor" strokeWidth="1.6"><path d="M12 3l2.2 4.9 5.3.6-4 3.6 1.1 5.2-4.6-2.7-4.6 2.7 1.1-5.2-4-3.6 5.3-.6L12 3z" /></svg>,
  },
  mcp: {
    label: 'MCP 服务', desc: 'JSON-RPC 工具发现与调用',
    icon: <svg viewBox="0 0 24 24" className="w-5 h-5" fill="none" stroke="currentColor" strokeWidth="1.6"><rect x="3" y="5" width="18" height="14" rx="2" /><path d="M7 10l2.5 2.5L7 15M12.5 15H17" /></svg>,
  },
  bot: {
    label: 'AI Bot', desc: '汇总 / 通知 / 报告',
    icon: <svg viewBox="0 0 24 24" className="w-5 h-5" fill="none" stroke="currentColor" strokeWidth="1.6"><rect x="5" y="8" width="14" height="10" rx="2" /><path d="M12 8V4M9 4h6" /><circle cx="9.5" cy="13" r="1" /><circle cx="14.5" cy="13" r="1" /></svg>,
  },
};
const PERM_LABEL: Record<string, string> = { scan: '扫描', network: '网络', llm: '大模型', read_html: '读页面', mcp: 'MCP', notify: '通知' };
const EXT_COLOR: Record<string, CatColor> = { agent: 'blue', skill: 'green', mcp: 'purple', bot: 'orange' };

export function ExtPage() {
  const toast = useToast();
  const [meta, setMeta] = useState<ExtMeta | null>(null);
  const [list, setList] = useState<Extension[]>([]);
  const [audit, setAudit] = useState<AuditEntry[]>([]);
  const [formExt, setFormExt] = useState<Extension | null | undefined>(undefined); // undefined=closed, null=new
  const [invokeExt, setInvokeExt] = useState<Extension | null>(null);
  const [discover, setDiscover] = useState<{ ext: Extension; caps: unknown } | null>(null);
  const [busyId, setBusyId] = useState('');
  const [delExt, setDelExt] = useState<Extension | null>(null);

  const loadAudit = async () => {
    try {
      const d = await api<{ entries: AuditEntry[] }>('/api/audit?limit=100');
      setAudit(d.entries);
    } catch { /* 审计加载失败不阻塞主界面 */ }
  };
  const load = async () => {
    try {
      const m = meta || await api<ExtMeta>('/api/ext/meta');
      if (!meta) setMeta(m);
      const d = await api<{ extensions: Extension[] }>('/api/ext/list');
      setList(d.extensions);
      loadAudit();
    } catch (e: any) { toast('扩展加载失败：' + e.message); }
  };
  useEffect(() => { load(); }, []);

  const toggle = async (ext: Extension, enabled: boolean) => {
    try {
      await api('/api/ext/update', { id: ext.id, enabled });
      toast(enabled ? '已启用' : '已禁用');
      load();
    } catch (e: any) { toast(e.message); load(); }
  };
  const remove = async (ext: Extension) => {
    try { await api('/api/ext/remove', { id: ext.id }); toast('已删除'); load(); }
    catch (e: any) { toast(e.message); }
  };
  const doDiscover = async (ext: Extension) => {
    setBusyId(ext.id);
    try {
      const r = await api<{ capabilities: unknown }>('/api/ext/discover', { id: ext.id });
      setDiscover({ ext, caps: r.capabilities });
    } catch (e: any) { toast('发现失败：' + e.message); }
    setBusyId('');
  };

  return (
    <div className="p-6 max-w-[1060px] mx-auto">
      <PageHeader id="ext" />

      <div className="grid grid-cols-4 gap-3 mb-4">
        {Object.entries(EXT_TYPE).map(([t, m]) => {
          const n = list.filter((e) => e.type === t).length;
          const on = list.filter((e) => e.type === t && e.enabled).length;
          return (
            <StatCard key={t} color={EXT_COLOR[t]} icon={m.icon} label={m.label} value={n}
              sub={`${m.desc} · 启用 ${on}`} />
          );
        })}
      </div>

      <Section
        title="已注册扩展"
        desc="智能体 / SKILL / MCP / Bot 统一注册、配置与权限管理；开关即时生效，所有调用写入审计日志。"
        actions={<>
          <button className="btn-ghost !h-8 !text-xs" onClick={load}>刷新</button>
          <button className="btn !h-8 !text-xs" onClick={() => setFormExt(null)}>+ 接入第三方扩展</button>
        </>}
        bodyClass=""
      >
        <div className="divide-y divide-line">
          {list.map((e) => (
            <div key={e.id} className="ext-row">
              <span className={`tile-sm cat-${EXT_COLOR[e.type] || 'blue'}`}>{EXT_TYPE[e.type]?.icon}</span>
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2 flex-wrap">
                  <span className="text-sm font-semibold text-ink">{e.name}</span>
                  <VerPill>v{e.version}</VerPill>
                  <IdBadge>{e.id}</IdBadge>
                </div>
                {e.description && <div className="text-xs text-ink-2 mt-1 truncate">{e.description}</div>}
                <div className="flex flex-wrap gap-1.5 mt-1.5">
                  <Tag kind="purple">{e.transport}</Tag>
                  {e.builtin && <Tag kind="gray">内置</Tag>}
                  {!e.enabled && <Tag kind="orange">已禁用</Tag>}
                  {(e.permissions || []).map((p) => <Tag key={p} kind="green">{PERM_LABEL[p] || p}</Tag>)}
                </div>
              </div>
              <div className="flex items-center gap-2 shrink-0">
                <Switch checked={e.enabled} onChange={(v) => toggle(e, v)} title="启用 / 禁用" />
                <button className="btn-ghost !h-7 !px-2.5 !text-xs" disabled={busyId === e.id} onClick={() => doDiscover(e)}>发现</button>
                <button className="btn-ghost !h-7 !px-2.5 !text-xs" onClick={() => setInvokeExt(e)}>调用</button>
                <button className="btn-ghost !h-7 !px-2.5 !text-xs" onClick={() => setFormExt(e)}>配置</button>
                {!e.builtin && <button className="btn-danger !h-7 !px-2.5 !text-xs" onClick={() => setDelExt(e)}>删除</button>}
              </div>
            </div>
          ))}
          {!list.length && <div className="px-5 py-8 text-center text-xs text-ink-3">暂无扩展，点击右上角「接入第三方扩展」开始</div>}
        </div>
      </Section>

      <Section
        title="审计日志"
        desc="最近 100 条扩展调用 / 编排运行记录"
        actions={<button className="btn-ghost !h-8 !text-xs" onClick={loadAudit}>刷新</button>}
        bodyClass="px-5 py-3"
      >
        <div className="max-h-64 overflow-auto space-y-1">
          {audit.map((a, i) => (
            <div key={i} className="flex items-center gap-2 text-xs font-mono">
              <span className="text-ink-3 shrink-0">{(a.ts || '').slice(11, 19)}</span>
              <span className={a.ok === false ? 'text-danger' : 'text-ok'}>{a.ok === false ? '✕' : '✓'}</span>
              <span className="text-ink-2 truncate">
                {a.action} {a.extId || a.pipelineId || ''}{a.ms != null ? ` · ${a.ms}ms` : ''}{a.error ? ' · ' + a.error : ''}
              </span>
            </div>
          ))}
          {!audit.length && <p className="text-xs text-ink-3">暂无审计记录</p>}
        </div>
      </Section>

      {formExt !== undefined && meta && (
        <ExtForm meta={meta} ext={formExt} onClose={() => setFormExt(undefined)} onSaved={() => { setFormExt(undefined); load(); }} />
      )}
      {invokeExt && <InvokeModal ext={invokeExt} onClose={() => setInvokeExt(null)} onDone={loadAudit} />}
      {discover && (
        <Modal title={`发现 · ${discover.ext.name}`} onClose={() => setDiscover(null)} wide>
          {Array.isArray(discover.caps) ? (
            <div className="space-y-2">
              <p className="text-[13px] text-ink-2">该 MCP 服务提供 {(discover.caps as any[]).length} 个工具：</p>
              {(discover.caps as any[]).map((t, i) => (
                <p key={i} className="text-xs text-ink-2"><span className="badge cat mr-2">{t.name}</span>{t.description || ''}</p>
              ))}
            </div>
          ) : (
            <pre className="code">{JSON.stringify(discover.caps, null, 2)}</pre>
          )}
        </Modal>
      )}
      {delExt && (
        <Confirm
          title="删除扩展" danger okText="删除"
          message={`删除扩展「${delExt.name}」？此操作不可恢复。`}
          onOk={() => remove(delExt)} onClose={() => setDelExt(null)}
        />
      )}
    </div>
  );
}

function ExtForm({ meta, ext, onClose, onSaved }: { meta: ExtMeta; ext: Extension | null; onClose: () => void; onSaved: () => void }) {
  const toast = useToast();
  const isEdit = !!ext;
  const [type, setType] = useState(ext?.type || 'agent');
  const [transport, setTransport] = useState(ext?.transport || 'http');
  const [id, setId] = useState(ext?.id || '');
  const [name, setName] = useState(ext?.name || '');
  const [version, setVersion] = useState(ext?.version || '0.1.0');
  const [description, setDescription] = useState(ext?.description || '');
  const [endpoint, setEndpoint] = useState(ext?.endpoint || '');
  const [command, setCommand] = useState(ext?.command || '');
  const [args, setArgs] = useState((ext?.args || []).join(','));
  const [module, setModule] = useState(ext?.module || '');
  const [perms, setPerms] = useState<string[]>(ext?.permissions || []);
  const [config, setConfig] = useState(JSON.stringify(ext?.config || {}, null, 2));
  const [saving, setSaving] = useState(false);

  const save = async () => {
    let cfg: unknown;
    try { cfg = JSON.parse(config || '{}'); } catch { toast('Config 不是合法 JSON'); return; }
    const body = {
      id: id.trim(), type, transport, name: name.trim(), version: version.trim(), description: description.trim(),
      endpoint: endpoint.trim(), command: command.trim(),
      args: args.split(',').map((s) => s.trim()).filter(Boolean),
      module: module.trim(), permissions: perms, config: cfg,
    };
    setSaving(true);
    try {
      if (isEdit) await api('/api/ext/update', { ...body, id: ext!.id });
      else await api('/api/ext/register', body);
      toast(isEdit ? '配置已保存' : '扩展已注册');
      onSaved();
    } catch (e: any) { toast(e.message); }
    setSaving(false);
  };

  return (
    <Modal title={isEdit ? '配置扩展' : '接入第三方扩展'} onClose={onClose} wide>
      <div className="grid grid-cols-2 gap-3">
        <label><span className="label">类型</span>
          <select className="input" value={type} disabled={isEdit} onChange={(e) => setType(e.target.value)}>
            {meta.types.map((t) => <option key={t} value={t}>{EXT_TYPE[t]?.label || t}</option>)}
          </select>
        </label>
        <label><span className="label">传输</span>
          <select className="input" value={transport} disabled={isEdit} onChange={(e) => setTransport(e.target.value)}>
            {meta.transports.filter((t) => t !== 'builtin').map((t) => <option key={t} value={t}>{t}</option>)}
          </select>
        </label>
        <label><span className="label">扩展 ID（唯一）</span>
          <input className="input" value={id} disabled={isEdit} onChange={(e) => setId(e.target.value)} placeholder="如 my-sqli-agent" />
        </label>
        <label><span className="label">名称</span>
          <input className="input" value={name} onChange={(e) => setName(e.target.value)} />
        </label>
        <label><span className="label">版本</span>
          <input className="input" value={version} onChange={(e) => setVersion(e.target.value)} />
        </label>
        <label><span className="label">描述</span>
          <input className="input" value={description} onChange={(e) => setDescription(e.target.value)} />
        </label>
      </div>
      {(transport === 'http' || transport === 'openai') && (
        <label className="block mt-3"><span className="label">Endpoint（http(s) 地址）</span>
          <input className="input" value={endpoint} onChange={(e) => setEndpoint(e.target.value)} placeholder="https://你的扩展服务.example" />
        </label>
      )}
      {transport === 'stdio' && (
        <>
          <label className="block mt-3"><span className="label">Command（stdio MCP 启动命令）</span>
            <input className="input" value={command} onChange={(e) => setCommand(e.target.value)} placeholder="node" />
          </label>
          <label className="block mt-3"><span className="label">Args（逗号分隔）</span>
            <input className="input" value={args} onChange={(e) => setArgs(e.target.value)} placeholder="server.mjs,--port,3000" />
          </label>
        </>
      )}
      {transport === 'module' && (
        <label className="block mt-3"><span className="label">Module 路径（必须位于 {meta.skillsDir}）</span>
          <input className="input" value={module} onChange={(e) => setModule(e.target.value)} placeholder={`${meta.skillsDir}\\my-skill.mjs`} />
        </label>
      )}
      <div className="mt-3">
        <span className="label">权限</span>
        <div className="flex flex-wrap gap-x-4 gap-y-1.5">
          {meta.permissions.map((p) => (
            <label key={p} className="flex items-center gap-1.5 text-xs text-ink-2 cursor-pointer">
              <input type="checkbox" className="accent-[#1677ff]" checked={perms.includes(p)}
                onChange={(e) => setPerms(e.target.checked ? [...perms, p] : perms.filter((x) => x !== p))} />
              {PERM_LABEL[p] || p}
            </label>
          ))}
        </div>
      </div>
      <label className="block mt-3"><span className="label">Config（JSON，如模型 / 系统提示词 / 工具名）</span>
        <textarea className="textarea font-mono !text-xs" rows={4} value={config} onChange={(e) => setConfig(e.target.value)} />
      </label>
      <div className="flex gap-2 mt-4">
        <button className="btn" disabled={saving} onClick={save}>{isEdit ? '保存配置' : '注册扩展'}</button>
        <button className="btn-ghost" onClick={onClose}>取消</button>
      </div>
      <p className="text-[11px] text-ink-3 mt-3 leading-relaxed">
        权限控制：调用前必须授予对应权限（扫描 / 网络 / 大模型 / 读页面 / MCP / 通知）。module 技能仅限本机技能目录；所有调用写入审计日志。
      </p>
    </Modal>
  );
}

function InvokeModal({ ext, onClose, onDone }: { ext: Extension; onClose: () => void; onDone: () => void }) {
  const toast = useToast();
  const isMcp = ext.type === 'mcp';
  const [url, setUrl] = useState(location.origin + '/demo');
  const [auth, setAuth] = useState(false);
  const [input, setInput] = useState(isMcp ? '{\n  "tool": "",\n  "args": {}\n}' : '{}');
  const [out, setOut] = useState('');
  const [running, setRunning] = useState(false);

  const run = async () => {
    let parsed: unknown;
    try { parsed = JSON.parse(input || '{}'); } catch { toast('Input 不是合法 JSON'); return; }
    const u = url.trim();
    if (!isLocalUrl(u) && !auth) { toast('请先勾选授权确认'); return; }
    setRunning(true);
    setOut('调用中…');
    try {
      const r = await api<{ ms: number; findings?: unknown[] }>('/api/ext/invoke', { id: ext.id, input: parsed, ctx: { url: u }, authorized: isLocalUrl(u) || auth });
      setOut(JSON.stringify(r, null, 2));
      toast(`调用成功 · ${r.ms}ms · ${(r.findings || []).length} 条发现`);
      onDone();
    } catch (e: any) {
      setOut('失败：' + e.message);
      onDone();
    }
    setRunning(false);
  };

  return (
    <Modal title={`调用测试 · ${ext.name}`} onClose={onClose} wide>
      <label className="block"><span className="label">目标 URL（本地演示无需授权）</span>
        <input className="input" value={url} onChange={(e) => setUrl(e.target.value)} />
      </label>
      <label className="flex items-center gap-2 text-[13px] text-ink-2 cursor-pointer mt-3">
        <input type="checkbox" className="accent-[#1677ff]" checked={auth} onChange={(e) => setAuth(e.target.checked)} />
        我已获得该网站测试授权
      </label>
      <label className="block mt-3"><span className="label">Input（JSON{isMcp ? '，MCP 需含 tool 与 args' : ''}）</span>
        <textarea className="textarea font-mono !text-xs" rows={5} value={input} onChange={(e) => setInput(e.target.value)} />
      </label>
      <div className="mt-3"><button className="btn" disabled={running} onClick={run}>执行调用</button></div>
      {out && <pre className="code mt-3 !max-h-72">{out}</pre>}
    </Modal>
  );
}