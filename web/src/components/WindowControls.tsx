import { useEffect, useMemo, useState } from 'react';
import { tauriWindow } from '../lib/tauri';

export function WindowControls() {
  const win = useMemo(() => tauriWindow(), []);
  const [max, setMax] = useState(false);

  useEffect(() => {
    if (!win) return;
    let alive = true;
    let un: (() => void) | undefined;
    const sync = () => win.isMaximized().then((m) => { if (alive) setMax(m); }).catch(() => {});
    sync();
    win.onResized(sync).then((u) => { un = u; }).catch(() => {});
    return () => { alive = false; un?.(); };
  }, [win]);

  if (!win) return null;
  return (
    <div className="flex items-stretch shrink-0">
      <button className="win-btn" title="最小化" aria-label="最小化" onClick={() => win.minimize()}>
        <svg viewBox="0 0 24 24" className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="1.5"><path d="M5 12h14" /></svg>
      </button>
      <button className="win-btn" title={max ? '还原' : '最大化'} aria-label={max ? '还原' : '最大化'} onClick={() => win.toggleMaximize()}>
        {max ? (
          <svg viewBox="0 0 24 24" className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="1.5"><rect x="5" y="8" width="11" height="11" rx="1" /><path d="M8 8V6a1 1 0 0 1 1-1h9a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1h-2" /></svg>
        ) : (
          <svg viewBox="0 0 24 24" className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="1.5"><rect x="5" y="5" width="14" height="14" rx="1" /></svg>
        )}
      </button>
      <button className="win-btn close" title="关闭" aria-label="关闭" onClick={() => win.close()}>
        <svg viewBox="0 0 24 24" className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="1.5"><path d="M6 6l12 12M18 6L6 18" /></svg>
      </button>
    </div>
  );
}