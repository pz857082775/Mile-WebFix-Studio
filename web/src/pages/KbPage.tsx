import { useEffect, useMemo, useState } from 'react';
import { api } from '../lib/api';
import { CAT, CAT_COLOR, SEV } from '../lib/format';
import type { Finding, Rule } from '../lib/types';
import { PageHeader } from '../components/PageHeader';
import { Ico, Section, Tag } from '../components/ui';

const RULE_ICON = 'M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8l-5-5Zm0 0v5h5M9 13h6M9 17h6';

export function KbPage({ rules, onFix }: { rules: Rule[]; onFix: (f: Finding, url?: string) => void }) {
  const [q, setQ] = useState('');
  const [cat, setCat] = useState('all');
  const [learned, setLearned] = useState<Rule[]>([]);
  const [learnError, setLearnError] = useState('');
  useEffect(() => {
    const load = () => api<{ entries: Rule[] }>('/api/kb/learned').then(d => { setLearned(d.entries); setLearnError(''); }).catch(e => setLearnError(e.message));
    void load();
    window.addEventListener('wf-knowledge-updated', load);
    return () => window.removeEventListener('wf-knowledge-updated', load);
  }, []);
  const learnedList = learned.filter(r => (cat === 'all' || r.category === cat) && (!q.trim() || (r.title + r.rootCause + r.id + (r.test || '')).toLowerCase().includes(q.trim().toLowerCase())));

  const counts = useMemo(() => {
    const c: Record<string, number> = { all: rules.length };
    for (const r of rules) c[r.category] = (c[r.category] || 0) + 1;
    return c;
  }, [rules]);

  const list = useMemo(() => {
    const ql = q.trim().toLowerCase();
    return rules.filter((r) =>
      (cat === 'all' || r.category === cat) &&
      (!ql || (r.title + r.rootCause + r.id + (r.test || '') + (CAT[r.category] || '')).toLowerCase().includes(ql)));
  }, [rules, q, cat]);

  const use = (r: Rule) => {
    onFix({
      ruleId: r.id, title: r.title, severity: r.severity, category: r.category,
      rootCause: r.rootCause, fix: r.fix, verify: r.verify, guard: r.guard,
      steps: '（请填写实际复现步骤）', evidence: '（请粘贴截图说明 / 请求响应 / 日志证据）',
    });
  };

  return (
    <div className="p-6 max-w-[1080px] mx-auto">
      <PageHeader id="kb" />
      <Section
        title="规则检索"
        desc="覆盖 OWASP Top10 / API Top10 / CWE Top25 与界面 / 功能 / 逻辑 / 数据 / 算法 / 性能 / 无障碍。自动规则来自体检与爬行；「挖掘模板」含挖掘方法，供人工与 AI 深度挖掘；运行时规则对应巡检抓取。"
      >
        <input className="input" placeholder="搜索规则，如：XSS / 越权 / 金额 / Cookie / 控制台 / 竞态 / 缓存"
          value={q} onChange={(e) => setQ(e.target.value)} />
        <div className="flex flex-wrap gap-2 mt-3">
          {[['all', '全部'], ...Object.entries(CAT)].map(([id, name]) => (
            <button key={id} className={`chip ${cat === id ? 'on' : ''}`} onClick={() => setCat(id)}>
              {name} {counts[id] || 0}
            </button>
          ))}
        </div>
        <p className="text-xs text-ink-3 mt-3">当前显示 {list.length} / {rules.length} 条</p>
      </Section>

      <Section title={`AI 学习知识 · ${learnedList.length}`} desc="AI 生成，未经验证，仅作人工研判和修复参考；不代表已新增自动检测能力。">
        {learnError && <p className="text-danger text-xs">知识库读取失败：{learnError}</p>}
        {learnedList.length === 0 && <p className="text-xs text-ink-3">暂无匹配的 AI 知识条目。</p>}
        {learnedList.map(r => <details key={r.id} className="border-b border-line py-2"><summary className="cursor-pointer text-sm">{r.title} · {SEV[r.severity] || r.severity}</summary><p className="text-xs text-ink-2 my-2">{r.rootCause}</p><p className="text-xs text-ink-3">{r.test}</p><button className="btn-ghost !h-8 !text-xs" onClick={() => use(r)}>在修复工作台使用</button></details>)}
      </Section>
      <Section title={`规则与挖掘模板 · ${list.length}`} bodyClass="">
        <div className="divide-y divide-line">
          {list.map((r) => (
            <details key={r.id} className="group">
              <summary className="ext-row cursor-pointer list-none select-none">
                <span className={`tile-sm cat-${CAT_COLOR[r.category] || 'blue'}`}><Ico d={RULE_ICON} /></span>
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2 flex-wrap">
                    <span className={`badge ${r.severity}`}>{SEV[r.severity] || r.severity}</span>
                    <span className="text-sm font-semibold text-ink">{r.title}</span>
                    {r.manual && <Tag kind="purple">挖掘模板</Tag>}
                  </div>
                  <div className="text-xs text-ink-3 mt-1 truncate">{CAT[r.category] || r.category} · {r.rootCause}</div>
                </div>
                <svg viewBox="0 0 24 24" className="w-4 h-4 text-ink-3 shrink-0 transition-transform group-open:rotate-180" fill="none" stroke="currentColor" strokeWidth="2"><path d="m6 9 6 6 6-6" /></svg>
              </summary>
              <div className="px-5 pb-4 pt-3 space-y-2.5 border-t border-line bg-[#fafbfc]">
                <div><h4 className="text-xs font-semibold text-ink-2 mb-1">根因分析</h4><p className="text-xs text-ink-2 leading-relaxed">{r.rootCause}</p></div>
                {r.test && <div><h4 className="text-xs font-semibold text-ink-2 mb-1">挖掘 / 复现方法</h4><p className="text-xs text-ink-2 leading-relaxed">{r.test}</p></div>}
                <div><h4 className="text-xs font-semibold text-ink-2 mb-1">最简修复 · 前端</h4><pre className="code">{r.fix.frontend}</pre></div>
                <div><h4 className="text-xs font-semibold text-ink-2 mb-1">最简修复 · 后端</h4><pre className="code">{r.fix.backend}</pre></div>
                <div><h4 className="text-xs font-semibold text-ink-2 mb-1">验证步骤</h4><pre className="code">{r.verify.map((s, j) => `${j + 1}. ${s}`).join('\n')}</pre></div>
                <div><h4 className="text-xs font-semibold text-ink-2 mb-1">防护代码</h4><pre className="code">{r.guard}</pre></div>
                <div><h4 className="text-xs font-semibold text-ink-2 mb-1">建议 git diff</h4><pre className="code">{r.diff || '暂无预置补丁，可在修复工作台生成修复方案'}</pre></div>
                <button className="btn !h-8 !text-xs" onClick={() => use(r)}>在修复工作台使用</button>
              </div>
            </details>
          ))}
          {list.length === 0 && <div className="px-5 py-8 text-center text-sm text-ink-3">没有匹配的规则</div>}
        </div>
      </Section>
    </div>
  );
}