import { useEffect, useRef, useState } from 'react';
import { Sidebar } from './components/Sidebar';
import { TopBar } from './components/TopBar';
import { Landing } from './components/Landing';
import { StatusBar } from './components/StatusBar';
import { About } from './components/About';
import { Changelog } from './components/Changelog';
import { UpdateDialog, type UpdateCheck } from './components/UpdateDialog';
import { ToastProvider } from './components/Toast';
import { ContextMenu } from './components/ContextMenu';
import { api } from './lib/api';
import { onTauriEvent } from './lib/tauri';
import type { Finding, PageId, Rule } from './lib/types';
import { ScanPage } from './pages/ScanPage';
import { CrawlPage } from './pages/CrawlPage';
import { InspectPage } from './pages/InspectPage';
import { FixPage } from './pages/FixPage';
import { KbPage } from './pages/KbPage';
import { AiPage } from './pages/AiPage';
import { ExtPage } from './pages/ExtPage';
import { OrchPage } from './pages/OrchPage';

export interface FixRequest { finding: Finding; url: string; seq: number }

export default function App() {
  const [tabs, setTabs] = useState<PageId[]>([]);
  const [active, setActive] = useState<PageId | null>(null);
  const [rules, setRules] = useState<Rule[]>([]);
  const [fixReq, setFixReq] = useState<FixRequest | null>(null);
  const [collapsed, setCollapsed] = useState(false);
  const [about, setAbout] = useState(false);
  const [changelog, setChangelog] = useState(false);
  const [updateOpen, setUpdateOpen] = useState(false);
  const [updateInitial, setUpdateInitial] = useState<UpdateCheck | null>(null);
  const seq = useRef(0);

  useEffect(() => {
    api<{ rules: Rule[] }>('/api/rules').then((d) => setRules(d.rules)).catch(() => {});
  }, []);

  // 启动静默检测更新：只有「有更新且未被忽略」时才弹出，失败一律静默
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const d = await api<UpdateCheck>('/api/update/check');
        if (cancelled || !d.autoCheck || !d.hasUpdate) return;
        let dismissed = '';
        try { dismissed = localStorage.getItem('wf-update-dismissed') || ''; } catch { /* ignore */ }
        if (dismissed !== d.latest) { setUpdateInitial(d); setUpdateOpen(true); }
      } catch { /* 静默失败：不打扰用户 */ }
    })();
    return () => { cancelled = true; };
  }, []);

  const checkUpdate = () => { setAbout(false); setUpdateInitial(null); setUpdateOpen(true); };
  const dismissUpdate = (v: string) => { try { localStorage.setItem('wf-update-dismissed', v); } catch { /* ignore */ } };

  const openTab = (id: PageId) => {
    setTabs((t) => (t.includes(id) ? t : [...t, id]));
    setActive(id);
  };
  // 桌面托盘「设置软件」→ 打开 AI 设置页（网页版无此事件，自动忽略）
  useEffect(() => {
    const off = onTauriEvent('wf-navigate', (id) => {
      if (typeof id === 'string') openTab(id as PageId);
    });
    return () => { off?.(); };
  }, []);
  const closeTab = (id: PageId) => {
    const next = tabs.filter((x) => x !== id);
    setTabs(next);
    if (active === id) setActive(next.length ? next[next.length - 1] : null);
  };
  const closeOthers = (id: PageId) => {
    setTabs([id]);
    setActive(id);
  };
  const closeAll = () => {
    setTabs([]);
    setActive(null);
  };
  const onFix = (finding: Finding, url = '') => {
    setFixReq({ finding, url, seq: ++seq.current });
    openTab('fix');
  };

  const page = (id: PageId) => {
    switch (id) {
      case 'scan': return <ScanPage onFix={onFix} />;
      case 'crawl': return <CrawlPage onFix={onFix} />;
      case 'inspect': return <InspectPage rules={rules} onFix={onFix} />;
      case 'fix': return <FixPage rules={rules} request={fixReq} />;
      case 'kb': return <KbPage rules={rules} onFix={onFix} />;
      case 'ai': return <AiPage />;
      case 'ext': return <ExtPage />;
      case 'orch': return <OrchPage onFix={onFix} />;
    }
  };

  return (
    <ToastProvider>
      <div className="h-full flex flex-col overflow-hidden bg-white">
        <TopBar
          tabs={tabs} active={active}
          onActivate={setActive} onClose={closeTab}
          onCloseOthers={closeOthers} onCloseAll={closeAll}
          onNew={() => openTab('scan')}
        />
        <div className="flex-1 min-h-0 flex">
          <Sidebar
            active={active} onOpen={openTab}
            collapsed={collapsed} onCollapse={() => setCollapsed(!collapsed)}
            onAbout={() => setAbout(true)}
            onChangelog={() => setChangelog(true)}
            onCheckUpdate={checkUpdate}
          />
          <div className="flex-1 min-w-0 flex flex-col bg-white">
            <main className="flex-1 min-h-0 relative">
              {tabs.length === 0 ? (
                <Landing onOpen={openTab} onChangelog={() => setChangelog(true)} />
              ) : (
                tabs.map((id) => (
                  <div key={id} className={`absolute inset-0 overflow-auto ${active === id ? '' : 'hidden'}`}>
                    {page(id)}
                  </div>
                ))
              )}
            </main>
            <StatusBar active={active} tabCount={tabs.length} ruleCount={rules.length} onChangelog={() => setChangelog(true)} />
          </div>
        </div>
        {about && <About onClose={() => setAbout(false)} onChangelog={() => setChangelog(true)} onCheckUpdate={checkUpdate} />}
        {changelog && <Changelog onClose={() => setChangelog(false)} />}
        {updateOpen && <UpdateDialog initial={updateInitial} onClose={() => setUpdateOpen(false)} onDismiss={dismissUpdate} />}
        <ContextMenu />
      </div>
    </ToastProvider>
  );
}