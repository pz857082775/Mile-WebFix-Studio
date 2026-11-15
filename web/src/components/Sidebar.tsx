import { useState } from 'react';
import { PAGE_META } from '../lib/format';
import { PageIcon } from '../lib/icons';
import type { PageId } from '../lib/types';

const GROUPS: { head: string; ids: PageId[] }[] = [
  { head: '巡检', ids: ['scan', 'crawl', 'inspect'] },
  { head: '修复', ids: ['fix', 'kb'] },
  { head: '智能', ids: ['ai', 'ext', 'orch'] },
];

export function Sidebar({ active, onOpen, collapsed, onCollapse, onAbout, onChangelog, onCheckUpdate }: {
  active: PageId | null;
  onOpen: (id: PageId) => void;
  collapsed: boolean;
  onCollapse: () => void;
  onAbout: () => void;
  onChangelog: () => void;
  onCheckUpdate: () => void;
}) {
  const [hidden, setHidden] = useState<Record<string, boolean>>({});
  return (
    <aside style={{ width: collapsed ? 64 : 232 }} className="shrink-0 overflow-hidden bg-side border-r border-line flex flex-col h-full">
      <div className={`flex items-center gap-2.5 h-[52px] shrink-0 ${collapsed ? 'justify-center' : 'px-4'}`}>
        <img src="/logo.png" alt="WebFix Studio" className="w-8 h-8 rounded-[9px] shadow-chip shrink-0" />
        {!collapsed && (
          <>
            <div className="flex-1 min-w-0">
              <div className="text-[14px] font-bold leading-tight truncate">WebFix Studio</div>
              <div className="text-[10px] text-ink-3 truncate">BUG / 漏洞巡检修复</div>
            </div>
            <button className="icon-btn !w-7 !h-7" title="收起侧栏" aria-label="收起侧栏" onClick={onCollapse}>
              <svg viewBox="0 0 24 24" className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="1.8"><rect x="3" y="4" width="18" height="16" rx="2" /><path d="M9 4v16" /></svg>
            </button>
          </>
        )}
      </div>
      {collapsed && (
        <button className="icon-btn mx-auto mb-1" title="展开侧栏" aria-label="展开侧栏" onClick={onCollapse}>
          <svg viewBox="0 0 24 24" className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth="1.8"><rect x="3" y="4" width="18" height="16" rx="2" /><path d="M9 4v16m4-8h4m-2-2 2 2-2 2" /></svg>
        </button>
      )}
      <nav className="flex-1 px-3 overflow-auto pb-2">
        {GROUPS.map((g) => (
          <div key={g.head} className="mb-1.5">
            {!collapsed && (
              <button className="side-head" onClick={() => setHidden((h) => ({ ...h, [g.head]: !h[g.head] }))}>
                <span>{g.head}</span>
                <svg viewBox="0 0 24 24" className={`w-3 h-3 transition-transform duration-150 ${hidden[g.head] ? '-rotate-90' : ''}`} fill="none" stroke="currentColor" strokeWidth="2"><path d="m6 9 6 6 6-6" /></svg>
              </button>
            )}
            {!hidden[g.head] && (
              <div className="space-y-0.5">
                {g.ids.map((id) => (
                  <div
                    key={id}
                    className={`nav-item ${active === id ? 'on' : ''} ${collapsed ? 'justify-center !px-0' : ''}`}
                    title={PAGE_META[id][0]}
                    onClick={() => onOpen(id)}
                  >
                    <PageIcon id={id} className="w-4 h-4 shrink-0" />
                    {!collapsed && <span className="flex-1 truncate">{PAGE_META[id][0]}</span>}
                    {!collapsed && active === id && <span className="w-1.5 h-1.5 rounded-full bg-accent shrink-0" />}
                  </div>
                ))}
              </div>
            )}
          </div>
        ))}
      </nav>
      <div className="px-3 py-2 border-t border-line">
        <div className={`nav-item ${collapsed ? 'justify-center !px-0' : ''}`} title="检查更新" onClick={onCheckUpdate}>
          <svg viewBox="0 0 24 24" className="w-4 h-4 shrink-0" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M12 3v12m0 0 4-4m-4 4-4-4M4 17v2a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-2" /></svg>
          {!collapsed && <span className="flex-1">检查更新</span>}
        </div>
        <div className={`nav-item ${collapsed ? 'justify-center !px-0' : ''}`} title="关于 WebFix Studio" onClick={onAbout}>
          <svg viewBox="0 0 24 24" className="w-4 h-4 shrink-0" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round"><path d="M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20Zm0 9v6m0-9.5v-.5" /></svg>
          {!collapsed && <span className="flex-1">关于 WebFix</span>}
          {!collapsed && <button className="text-[10px] text-ink-3 hover:text-accent cursor-pointer" title="查看历史更新记录" onClick={(e) => { e.stopPropagation(); onChangelog(); }}>v1.2.1</button>}
        </div>
      </div>
    </aside>
  );
}