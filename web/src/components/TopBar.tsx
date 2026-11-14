import { useEffect, useRef, useState } from 'react';
import { PAGE_META } from '../lib/format';
import type { PageId } from '../lib/types';
import { WindowControls } from './WindowControls';

export function TopBar({ tabs, active, onActivate, onClose, onCloseOthers, onCloseAll, onNew }: {
  tabs: PageId[];
  active: PageId | null;
  onActivate: (id: PageId) => void;
  onClose: (id: PageId) => void;
  onCloseOthers: (id: PageId) => void;
  onCloseAll: () => void;
  onNew: () => void;
}) {
  const [menu, setMenu] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const h = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) setMenu(false);
    };
    document.addEventListener('mousedown', h);
    return () => document.removeEventListener('mousedown', h);
  }, []);

  return (
    <header data-tauri-drag-region className="h-10 shrink-0 bg-topbar border-b border-line flex items-stretch pl-1 select-none">
      <div data-tauri-drag-region className="flex-1 min-w-0 flex items-stretch overflow-x-auto [scrollbar-width:none]">
        {tabs.length === 0 && (
          <span data-tauri-drag-region className="self-center text-xs text-ink-3 pl-3 whitespace-nowrap">
            点击左侧菜单打开功能页签 · 支持多开 / 单个关闭 / 批量关闭
          </span>
        )}
        {tabs.map((id) => (
          <div
            key={id}
            className={`tab-item ${active === id ? 'on' : ''}`}
            onClick={() => onActivate(id)}
            title={PAGE_META[id][1]}
          >
            {active === id && <span className="w-1.5 h-1.5 rounded-full bg-accent shrink-0" />}
            <span>{PAGE_META[id][0]}</span>
            <button
              className="tab-x"
              aria-label={`关闭${PAGE_META[id][0]}`}
              onClick={(e) => { e.stopPropagation(); onClose(id); }}
            >
              <svg viewBox="0 0 24 24" className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2"><path d="M6 6l12 12M18 6L6 18" /></svg>
            </button>
          </div>
        ))}
        <button className="icon-btn self-center ml-1 shrink-0" title="新建页签（一键体检）" aria-label="新建页签" onClick={onNew}>
          <svg viewBox="0 0 24 24" className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="1.8"><path d="M12 5v14M5 12h14" /></svg>
        </button>
      </div>
      {tabs.length > 0 && (
        <div className="relative shrink-0 self-center mr-1.5" ref={menuRef}>
          <button className="btn-ghost !h-7 !px-2.5 !text-xs" onClick={() => setMenu(!menu)}>
            批量关闭
            <svg viewBox="0 0 24 24" className={`w-3 h-3 transition-transform ${menu ? 'rotate-180' : ''}`} fill="none" stroke="currentColor" strokeWidth="2.5"><path d="m6 9 6 6 6-6" /></svg>
          </button>
          {menu && (
            <div className="absolute right-0 top-9 card w-48 py-1.5 z-50 animate-popIn shadow-dialog">
              {active && (
                <button className="w-full text-left px-4 h-8 text-[13px] hover:bg-side transition-colors"
                  onClick={() => { onCloseOthers(active); setMenu(false); }}>
                  关闭其他页签
                </button>
              )}
              <button className="w-full text-left px-4 h-8 text-[13px] text-danger hover:bg-[#ffece8] transition-colors"
                onClick={() => { onCloseAll(); setMenu(false); }}>
                全部关闭（回到落地页）
              </button>
            </div>
          )}
        </div>
      )}
      <WindowControls />
    </header>
  );
}