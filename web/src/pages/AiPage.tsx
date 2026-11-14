import { useEffect, useState } from 'react';
import { api } from '../lib/api';
import type { AiConfig, AiProvider } from '../lib/types';
import { useToast } from '../components/Toast';
import { PageHeader } from '../components/PageHeader';
import { Ico, Section, StatCard, Switch } from '../components/ui';

export function AiPage() {
  const toast = useToast();
  const [providers, setProviders] = useState<AiProvider[]>([]);
  const [enabled, setEnabled] = useState(false);
  const [provider, setProvider] = useState('deepseek');
  const [baseUrl, setBaseUrl] = useState('');
  const [model, setModel] = useState('');
  const [models, setModels] = useState<string[]>([]);
  const [apiKey, setApiKey] = useState('');
  const [hasKey, setHasKey] = useState(false);
  const [keyTail, setKeyTail] = useState('');
  const [status, setStatus] = useState('');

  const presetOf = (id: string) => providers.find((p) => p.id === id);

  const load = async () => {
    try {
      const d = await api<{ config: AiConfig; providers: AiProvider[] }>('/api/ai/config');
      setProviders(d.providers);
      setEnabled(d.config.enabled);
      setProvider(d.config.provider || 'deepseek');
      setBaseUrl(d.config.baseUrl || presetOf(d.config.provider || 'deepseek')?.baseUrl || '');
      setModel(d.config.model || '');
      setHasKey(d.config.hasKey);
      setKeyTail(d.config.keyTail);
      setModels(presetOf(d.config.provider || 'deepseek')?.models || []);
    } catch (e: any) { toast('AI 配置加载失败：' + e.message); }
  };
  useEffect(() => { load(); }, []);

  const changeProvider = (id: string) => {
    setProvider(id);
    const p = presetOf(id);
    if (p?.baseUrl) setBaseUrl(p.baseUrl);
    setModels(p?.models || []);
    if (!model && p?.models[0]) setModel(p.models[0]);
  };

  const save = async () => {
    try {
      const body: Record<string, unknown> = { enabled, provider, baseUrl: baseUrl.trim(), model: model.trim() };
      if (apiKey.trim()) body.apiKey = apiKey.trim();
      const d = await api<{ config: AiConfig }>('/api/ai/config', body);
      setHasKey(d.config.hasKey);
      setKeyTail(d.config.keyTail);
      setApiKey('');
      setStatus(d.config.enabled
        ? `已保存并启用 AI 增强模式（${d.config.model || '未选模型'}）。体检/爬行/巡检结果区可用「AI 深度挖掘」。`
        : '已保存。当前为零成本模式，不联网大模型、不产生费用。');
      toast('AI 配置已保存');
    } catch (e: any) { setStatus('保存失败：' + e.message); }
  };

  const test = async () => {
    setStatus('正在测试连接…');
    try {
      const d = await api<{ model: string; reply: string }>('/api/ai/test', {});
      setStatus(`连接成功：模型 ${d.model} 回复「${d.reply}」`);
      toast('AI 连接成功');
    } catch (e: any) { setStatus('连接失败：' + e.message); }
  };

  const pullModels = async () => {
    setStatus('正在拉取模型列表…');
    try {
      const d = await api<{ models: string[] }>('/api/ai/models', { baseUrl: baseUrl.trim(), apiKey: apiKey.trim() });
      setModels(d.models);
      if (!model && d.models[0]) setModel(d.models[0]);
      setStatus(`拉取到 ${d.models.length} 个模型，已在「模型名称」下拉候选中。`);
    } catch (e: any) { setStatus('拉取失败：' + e.message); }
  };

  const clearKey = async () => {
    try {
      const d = await api<{ config: AiConfig }>('/api/ai/config', { clearKey: true });
      setHasKey(d.config.hasKey);
      setKeyTail(d.config.keyTail);
      setApiKey('');
      toast('本机密钥已清除');
    } catch (e: any) { toast(e.message); }
  };

  return (
    <div className="p-6 max-w-[860px] mx-auto">
      <PageHeader id="ai" />
      <div className="grid grid-cols-3 gap-3 mb-4">
        <StatCard color={enabled ? 'purple' : 'green'} label="运行模式" value={enabled ? 'AI 增强' : '零成本'}
          sub={enabled ? '大模型深度挖掘' : '本地规则 · 无费用'}
          icon={<Ico d="M12 3l2.2 4.9 5.3.6-4 3.6 1.1 5.2-4.6-2.7-4.6 2.7 1.1-5.2-4-3.6 5.3-.6L12 3Z" />} />
        <StatCard color="blue" label="服务商" value={presetOf(provider)?.name || provider || '未选择'}
          sub="OpenAI 兼容接口"
          icon={<Ico d="M4 7h16M4 12h16M4 17h10m4 0 2 2 4-4" />} />
        <StatCard color={hasKey ? 'green' : 'orange'} label="API 密钥" value={hasKey ? '已配置' : '未配置'}
          sub={hasKey ? `仅存本机 …${keyTail}` : '仅存本机 · 不上传'}
          icon={<Ico d="M15 7a4 4 0 1 1-3.9 5H7v3H4v-3l5.1-5.1A4 4 0 0 1 15 7Zm0 0 3 3m-2 1 2 2" />} />
      </div>
      <Section
        title="AI 增强配置"
        desc="默认零成本模式：全部功能本地规则运行，不联网大模型、不产生任何费用。启用 AI 增强后，体检 / 爬行 / 巡检可调用大模型深度挖掘逻辑 / 越权 / 资损等深层问题。密钥仅保存在本机 ~/.webfix-studio/ai-config.json，只发送到你配置的 API 地址。"
        actions={<button className="btn !h-8 !text-xs" onClick={save}>保存配置</button>}
      >
        <div className="flex items-center gap-3 mb-4">
          <Switch checked={enabled} onChange={setEnabled} title="启用 / 关闭 AI 增强" />
          <span className="text-[13px] text-ink-2">启用 AI 增强模式（关闭 = 零成本模式，不联网大模型、不产生费用）</span>
        </div>
        <div className="grid grid-cols-2 gap-3">
          <label>
            <span className="label">服务商</span>
            <select className="input" value={provider} onChange={(e) => changeProvider(e.target.value)}>
              {providers.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
            </select>
          </label>
          <label>
            <span className="label">模型名称</span>
            <input className="input" list="aiModelList" value={model} onChange={(e) => setModel(e.target.value)} placeholder="如 deepseek-chat" />
            <datalist id="aiModelList">{models.map((m) => <option key={m} value={m} />)}</datalist>
          </label>
        </div>
        <label className="block mt-3">
          <span className="label">API 地址（OpenAI 兼容）</span>
          <input className="input" value={baseUrl} onChange={(e) => setBaseUrl(e.target.value)} placeholder="https://api.deepseek.com/v1" />
        </label>
        <label className="block mt-3">
          <span className="label">API 密钥（仅存本机）</span>
          <input className="input" type="password" autoComplete="off" value={apiKey} onChange={(e) => setApiKey(e.target.value)}
            placeholder={hasKey ? `已保存（…${keyTail}），留空沿用` : 'sk-...（Ollama 本地无需密钥）'} />
        </label>
        <div className="flex flex-wrap gap-2 mt-4">
          <button className="btn-ghost" onClick={test}>测试连接</button>
          <button className="btn-ghost" onClick={pullModels}>拉取模型列表</button>
          <button className="btn-danger" onClick={clearKey}>清除密钥</button>
        </div>
        {status && <p className="text-[13px] text-ink-2 mt-3 animate-paneIn">{status}</p>}
      </Section>
    </div>
  );
}