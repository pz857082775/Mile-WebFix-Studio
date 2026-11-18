import { CAT, SEV } from '../lib/format';
import type { Finding } from '../lib/types';

export interface FindingFilter { query: string; category: string; severity: string }
export const emptyFindingFilter: FindingFilter = { query: '', category: '', severity: '' };

export function matchesFinding(f: Finding, filter: FindingFilter): boolean {
  const q = filter.query.trim().toLocaleLowerCase();
  const text = [f.title, f.ruleId, f.evidence, f.url, ...(f.urls || [])].filter(Boolean).join(' ').toLocaleLowerCase();
  return (!q || text.includes(q)) && (!filter.category || f.category === filter.category) && (!filter.severity || f.severity === filter.severity);
}

export function FindingFilters({ value, onChange, count, total }: { value: FindingFilter; onChange: (value: FindingFilter) => void; count: number; total: number }) {
  return <div className="card p-3 mb-4 flex flex-wrap items-end gap-3">
    <label className="flex-1 min-w-[200px]"><span className="label">搜索问题或页面</span><input className="input" value={value.query} onChange={e => onChange({ ...value, query: e.target.value })} placeholder="标题、证据、规则或网址" /></label>
    <label><span className="label">问题类别</span><select className="input !w-auto" value={value.category} onChange={e => onChange({ ...value, category: e.target.value })}><option value="">全部类别</option>{Object.entries(CAT).map(([id, name]) => <option key={id} value={id}>{name}</option>)}</select></label>
    <label><span className="label">风险等级</span><select className="input !w-auto" value={value.severity} onChange={e => onChange({ ...value, severity: e.target.value })}><option value="">全部等级</option>{Object.entries(SEV).map(([id, name]) => <option key={id} value={id}>{name}</option>)}</select></label>
    <span className="text-xs text-ink-3 pb-2">显示 {count} / {total} 条</span>
    <button className="btn-ghost !h-9" onClick={() => onChange(emptyFindingFilter)}>重置</button>
  </div>;
}
