import { useEffect, useRef, useState } from 'react';
import { Sidebar } from './components/Sidebar';
import { TopBar } from './components/TopBar';
import { Landing } from './components/Landing';
import { StatusBar } from './components/StatusBar';
import { About } from './components/About';
import { ToastProvider } from './components/Toast';
import { api } from './lib/api';
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
  const seq = useRef(0);

  useEffect(() => {
    api<{ rules: Rule[] }>('/api/rules').then((d) => setRules(d.rules)).catch(() => {});
  }, []);

  const openTab = (id: PageId) => {
    setTabs((t) => (t.includes(id) ? t : [...t, id]));
    setActive(id);
  };
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
          />
          <div className="flex-1 min-w-0 flex flex-col bg-white">
            <main className="flex-1 min-h-0 relative">
              {tabs.length === 0 ? (
                <Landing onOpen={openTab} />
              ) : (
                tabs.map((id) => (
                  <div key={id} className={`absolute inset-0 overflow-auto ${active === id ? '' : 'hidden'}`}>
                    {page(id)}
                  </div>
                ))
              )}
            </main>
            <StatusBar active={active} tabCount={tabs.length} ruleCount={rules.length} />
          </div>
        </div>
        {about && <About onClose={() => setAbout(false)} />}
      </div>
    </ToastProvider>
  );
}