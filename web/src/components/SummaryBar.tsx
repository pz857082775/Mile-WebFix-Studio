import type { Summary } from '../lib/types';
import { CAT, CAT_COLOR } from '../lib/format';
import { Ico, StatCard, Tag, type CatColor } from './ui';

export function SummaryBar({ summary, label }: { summary: Summary; label: string }) {
  const crit = summary.bySeverity.critical || 0;
  const high = summary.bySeverity.high || 0;
  const med = summary.bySeverity.medium || 0;
  const low = summary.bySeverity.low || 0;
  const cats = Object.entries(summary.byCategory);
  return (
    <div className="mb-4 animate-paneIn">
      <div className="grid grid-cols-4 gap-3">
        <StatCard color="blue" label="发现总数" value={summary.total} sub={label}
          icon={<Ico d="M9 5H7a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V7a2 2 0 0 0-2-2h-2M9 5a2 2 0 0 0 2 2h2a2 2 0 0 0 2-2M9 5a2 2 0 0 1 2-2h2a2 2 0 0 1 2 2m-6 9 2 2 4-4" />} />
        <StatCard color="red" label="严重" value={crit} sub="需立即修复"
          icon={<Ico d="M12 3 2.5 20h19L12 3Zm0 7v4m0 3v.5" />} />
        <StatCard color="orange" label="高危" value={high} sub="尽快修复"
          icon={<Ico d="M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20Zm0 6v5m0 3v.5" />} />
        <StatCard color="cyan" label="中 / 低危" value={med + low} sub={`中危 ${med} · 低危 ${low}`}
          icon={<Ico d="M12 3l7 3v6c0 4-3 7-7 9-4-2-7-5-7-9V6l7-3Zm-2 9 2 2 4-4" />} />
      </div>
      {cats.length > 0 && (
        <div className="flex flex-wrap gap-1.5 mt-3">
          {cats.map(([c, n]) => <Tag key={c} kind={(CAT_COLOR[c] || 'gray') as CatColor}>{CAT[c] || c} {n}</Tag>)}
        </div>
      )}
    </div>
  );
}