import { useEffect, useState } from 'react';
import { Modal } from './Modal';
import { api, copyText } from '../lib/api';
import { revealInFolder } from '../lib/tauri';

interface Asset { name: string; url: string; size: number }
export interface UpdateCheck {
  current: string; latest: string; hasUpdate: boolean; currentAhead?: boolean; notes: string; url: string;
  assets: Asset[]; publishedAt: string; source: string; repo: string; autoCheck: boolean; manifestUrl: string;
}
interface DownloadResult { ok: boolean; name: string; path: string; dir: string; size: number }

function versionLabel(version: string): string {
  return `v${version.trim().replace(/^v+/i, '')}`;
}

function fmtSize(n: number): string {
  if (!n) return '';
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

export function UpdateDialog({ onClose, initial, onDismiss }: {
  onClose: () => void;
  initial?: UpdateCheck | null;
  onDismiss?: (version: string) => void;
}) {
  const [phase, setPhase] = useState<'loading' | 'ready' | 'error'>(initial ? 'ready' : 'loading');
  const [data, setData] = useState<UpdateCheck | null>(initial ?? null);
  const [err, setErr] = useState('');
  const [selected, setSelected] = useState(0);
  const [downloading, setDownloading] = useState(false);
  const [dl, setDl] = useState<DownloadResult | null>(null);
  const [dlErr, setDlErr] = useState('');
  const [copied, setCopied] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [manifestUrl, setManifestUrl] = useState(initial?.manifestUrl ?? '');
  const [autoCheck, setAutoCheck] = useState(initial?.autoCheck ?? true);
  const [saving, setSaving] = useState(false);

  const check = async () => {
    setPhase('loading'); setErr(''); setDl(null); setDlErr(''); setCopied(false);
    try {
      const d = await api<UpdateCheck>('/api/update/check');
      setData(d); setManifestUrl(d.manifestUrl || ''); setAutoCheck(d.autoCheck !== false);
      setSelected(0); setPhase('ready');
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e)); setPhase('error');
    }
  };

  useEffect(() => { if (!initial) check(); /* eslint-disable-next-line react-hooks/exhaustive-deps */ }, []);

  const doDownload = async () => {
    const a = data?.assets?.[selected];
    if (!a) return;
    setDownloading(true); setDlErr(''); setCopied(false);
    try {
      const r = await api<DownloadResult>('/api/update/download', { url: a.url, name: a.name });
      setDl(r);
    } catch (e) {
      setDlErr(e instanceof Error ? e.message : String(e));
    } finally {
      setDownloading(false);
    }
  };

  const openFolder = async () => {
    if (!dl) return;
    const ok = await revealInFolder(dl.path);
    if (!ok) { await copyText(dl.dir); setCopied(true); }
  };

  const saveConfig = async () => {
    setSaving(true); setDlErr('');
    try {
      await api('/api/update/config', { manifestUrl: manifestUrl.trim(), autoCheck });
      await check();
    } catch (e) {
      setDlErr(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  const openPage = () => { if (data?.url) window.open(data.url, '_blank', 'noopener'); };
  const dismiss = () => { if (data?.hasUpdate) onDismiss?.(data.latest); onClose(); };

  const footer = phase === 'ready' && data?.hasUpdate ? (
    <>
      <button className="btn-ghost" onClick={dismiss}>暂不更新</button>
      {data.assets?.length ? (
        <button className="btn" disabled={downloading} onClick={doDownload}>
          {downloading ? '下载中…' : dl ? '重新下载' : '立即更新'}
        </button>
      ) : (
        <button className="btn" onClick={openPage}>打开下载页</button>
      )}
    </>
  ) : (
    <>
      <button className="btn-ghost" onClick={onClose}>关闭</button>
      <button className="btn" disabled={phase === 'loading'} onClick={check}>
        {phase === 'loading' ? '检测中…' : '重新检测'}
      </button>
    </>
  );

  return (
    <Modal title="软件更新" onClose={onClose} footer={footer}>
      {phase === 'loading' && (
        <div className="flex items-center gap-3 py-6 text-sm text-ink-2">
          <span className="w-4 h-4 rounded-full border-2 border-accent border-t-transparent animate-spin" />
          正在检测更新…
        </div>
      )}

      {phase === 'error' && (
        <div className="rounded-lg border border-line bg-side px-4 py-3 text-sm text-danger leading-relaxed">
          检测失败：{err}
          <div className="text-[11px] text-ink-3 mt-1">请检查网络，或在「更新设置」中配置可用的更新渠道。</div>
        </div>
      )}

      {phase === 'ready' && data && (
        <div className="space-y-4">
          <div className="flex items-center gap-3">
            <div className="flex-1 min-w-0">
              <div className="text-xs text-ink-3">当前版本</div>
              <div className="text-[15px] font-bold text-ink">{versionLabel(data.current)}</div>
            </div>
            <svg viewBox="0 0 24 24" className="w-5 h-5 text-ink-3 shrink-0" fill="none" stroke="currentColor" strokeWidth="1.8"><path d="M5 12h14m-6-6 6 6-6 6" /></svg>
            <div className="flex-1 min-w-0 text-right">
              <div className="text-xs text-ink-3">最新版本</div>
              <div className="text-[15px] font-bold text-accent">{versionLabel(data.latest)}</div>
            </div>
            <span className={`badge ${data.hasUpdate ? 'cat' : 'ok'} shrink-0`}>{data.hasUpdate ? '有更新' : data.currentAhead ? '本地版本较新' : '已是最新'}</span>
          </div>

          <div className="text-[11px] text-ink-3">
            更新源：{data.source === 'github' ? `GitHub Releases（${data.repo}）` : '自定义渠道'}
            {data.publishedAt && ` · 发布于 ${data.publishedAt.slice(0, 10)}`}
          </div>

          {data.hasUpdate && data.notes && (
            <div className="rounded-lg border border-line bg-side px-4 py-3">
              <div className="text-xs font-semibold text-ink-2 mb-1.5">更新说明</div>
              <div className="text-xs text-ink-2 leading-relaxed whitespace-pre-wrap max-h-[180px] overflow-auto">{data.notes}</div>
            </div>
          )}

          {data.hasUpdate && !!data.assets?.length && (
            <div className="space-y-1.5">
              <div className="text-xs font-semibold text-ink-2">选择更新包</div>
              {data.assets.map((a, i) => (
                <label key={a.url + i} className={`flex items-center gap-2.5 rounded-lg border px-3 py-2 cursor-pointer text-xs ${selected === i ? 'border-accent bg-accent-soft' : 'border-line hover:bg-side'}`}>
                  <input type="radio" name="wf-asset" checked={selected === i} onChange={() => setSelected(i)} />
                  <span className="flex-1 min-w-0 truncate text-ink-2">{a.name}</span>
                  <span className="text-ink-3 shrink-0">{fmtSize(a.size)}</span>
                </label>
              ))}
            </div>
          )}

          {data.hasUpdate && !data.assets?.length && (
            <div className="text-xs text-ink-3">该版本未提供直接下载包，请点击「打开下载页」前往发布页获取。</div>
          )}

          {downloading && (
            <div className="flex items-center gap-2.5 text-xs text-ink-2">
              <span className="w-3.5 h-3.5 rounded-full border-2 border-accent border-t-transparent animate-spin" />
              正在下载更新包到本地…（完成后请手动安装）
            </div>
          )}

          {dlErr && <div className="text-xs text-danger leading-relaxed">下载失败：{dlErr}</div>}

          {dl && (
            <div className="rounded-lg border border-ok/40 bg-[#f0faf2] px-4 py-3 space-y-2">
              <div className="text-xs font-semibold text-ok">已下载到本地，请手动安装</div>
              <div className="text-[11px] text-ink-2 break-all">{dl.path}</div>
              <div className="flex items-center gap-2">
                <button className="btn !h-7 !px-3 !text-xs" onClick={openFolder}>打开文件夹</button>
                <button className="btn-ghost !h-7 !px-3 !text-xs" onClick={async () => { await copyText(dl.path); setCopied(true); }}>复制路径</button>
                {copied && <span className="text-[11px] text-ink-3">已复制</span>}
              </div>
              <div className="text-[11px] text-ink-3">安装前请关闭本软件；解压 / 运行安装包后即可完成更新。</div>
            </div>
          )}

          {!data.hasUpdate && (
            <div className="text-xs text-ink-2">{data.currentAhead ? '当前版本高于更新源的最新发布版本，暂不需要更新。' : '当前已是最新版本，无需更新。'}</div>
          )}

          <div className="border-t border-line pt-3">
            <button className="text-[11px] text-ink-3 hover:text-accent cursor-pointer inline-flex items-center gap-1" onClick={() => setShowSettings((s) => !s)}>
              <svg viewBox="0 0 24 24" className="w-3.5 h-3.5" fill="none" stroke="currentColor" strokeWidth="1.8"><path d="M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z" /><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-2.9 1.2 2 2 0 1 1-4 0 1.7 1.7 0 0 0-2.9-1.2l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1A1.7 1.7 0 0 0 4.6 15a2 2 0 1 1 0-4 1.7 1.7 0 0 0 1.2-2.9l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1A1.7 1.7 0 0 0 11.5 4a2 2 0 1 1 4 0 1.7 1.7 0 0 0 2.9 1.2l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.9 2 2 0 1 1 0 4Z" /></svg>
              更新设置{showSettings ? '（收起）' : ''}
            </button>
            {showSettings && (
              <div className="mt-2.5 space-y-2.5">
                <label className="block text-[11px] text-ink-3">
                  自定义更新渠道（manifest JSON 地址，留空使用 GitHub Releases）
                  <input
                    className="input mt-1 !h-8 !text-xs" placeholder="https://example.com/webfix-manifest.json"
                    value={manifestUrl} onChange={(e) => setManifestUrl(e.target.value)}
                  />
                </label>
                <label className="flex items-center gap-2 text-xs text-ink-2 cursor-pointer">
                  <input type="checkbox" checked={autoCheck} onChange={(e) => setAutoCheck(e.target.checked)} />
                  启动时自动检测更新
                </label>
                <button className="btn !h-7 !px-3 !text-xs" disabled={saving} onClick={saveConfig}>{saving ? '保存中…' : '保存并检测'}</button>
              </div>
            )}
          </div>
        </div>
      )}
    </Modal>
  );
}