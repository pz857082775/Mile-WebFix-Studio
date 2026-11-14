import type { ReactNode } from 'react';

export type CatColor = 'blue' | 'green' | 'pink' | 'purple' | 'orange' | 'red' | 'cyan' | 'geek';

export function Ico({ d, className = 'w-5 h-5' }: { d: string; className?: string }) {
  return (
    <svg viewBox="0 0 24 24" className={className} fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
      <path d={d} />
    </svg>
  );
}

export function StatCard({ color, icon, label, value, sub }: {
  color: CatColor; icon: ReactNode; label: string; value: ReactNode; sub?: ReactNode;
}) {
  return (
    <div className={`cat-card cat-${color} stat animate-paneIn`}>
      <div className="cat-tile">{icon}</div>
      <div className="cat-num" title={typeof value === 'string' || typeof value === 'number' ? String(value) : undefined}>{value}</div>
      <div className="cat-label">{label}</div>
      {sub != null && <span className="cat-desc">{sub}</span>}
    </div>
  );
}

export function Switch({ checked, onChange, disabled, title }: {
  checked: boolean; onChange: (v: boolean) => void; disabled?: boolean; title?: string;
}) {
  return (
    <label className="switch" title={title}>
      <input type="checkbox" checked={checked} disabled={disabled} onChange={(e) => onChange(e.target.checked)} />
      <i />
    </label>
  );
}

export function Tag({ kind = 'gray', children }: {
  kind?: CatColor | 'gray'; children: ReactNode;
}) {
  return <span className={`tag ${kind}`}>{children}</span>;
}

export function VerPill({ children }: { children: ReactNode }) {
  return <span className="ver-pill">{children}</span>;
}

export function IdBadge({ children }: { children: ReactNode }) {
  return <code className="id-badge">{children}</code>;
}

export function Section({ title, desc, actions, children, bodyClass = 'p-5' }: {
  title: ReactNode; desc?: ReactNode; actions?: ReactNode; children: ReactNode; bodyClass?: string;
}) {
  return (
    <div className="card mb-4 animate-paneIn">
      <div className="flex items-center gap-3 px-5 pt-4 pb-3">
        <h3 className="text-[15px] font-semibold text-ink flex-1 min-w-0 truncate">{title}</h3>
        {actions && <div className="flex items-center gap-2 shrink-0">{actions}</div>}
      </div>
      {desc != null && <p className="px-5 pb-3 -mt-1 text-xs text-ink-3 leading-relaxed">{desc}</p>}
      <div className={`border-t border-line ${bodyClass}`}>{children}</div>
    </div>
  );
}