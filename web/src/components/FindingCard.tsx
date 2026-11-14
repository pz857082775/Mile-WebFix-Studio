import { useState } from 'react';
import type { Finding } from '../lib/types';
import { CAT, SEV } from '../lib/format';

export function FindingCard({ f, index, onFix }: { f: Finding; index: number; onFix?: (f: Finding) => void }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="card p-4 mb-3 animate-paneIn" style={{ animationDelay: `${Math.min(index, 8) * 40}ms` }}>
      <div className="flex items-start gap-3">
        <div className="flex flex-col gap-1.5 shrink-0 pt-0.5">
          <span className={`badge ${f.severity}`}>{SEV[f.severity] || f.severity}</span>
          <span className="badge cat">{CAT[f.category] || f.category}</span>
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 flex-wrap">
            <h4 className="text-sm font-semibold text-ink">{f.title}</h4>
            {f.source === 'ai' && <span className="badge ai">AI 挖掘</span>}
            {f.ruleId && <code className="text-[11px] text-ink-3 bg-side px-1.5 py-0.5 rounded">{f.ruleId}</code>}
          </div>
          {f.url && <div className="text-xs text-ink-3 mt-1 truncate" title={f.url}>{f.url}</div>}
          {f.urls && f.urls.length > 1 && (
            <div className="text-xs text-ink-3 mt-1">影响 {f.urls.length} 个页面</div>
          )}
          {f.evidence && (
            <pre className="code mt-2 !max-h-40">{f.evidence}</pre>
          )}
          {open && (
            <div className="mt-3 space-y-2 animate-paneIn">
              {f.rootCause && <div className="text-xs text-ink-2"><b>根因：</b>{f.rootCause}</div>}
              {f.fix && (
                <>
                  <div className="text-xs font-medium text-ink-2">最简修复 · 前端</div>
                  <pre className="code">{f.fix.frontend}</pre>
                  <div className="text-xs font-medium text-ink-2">最简修复 · 后端</div>
                  <pre className="code">{f.fix.backend}</pre>
                </>
              )}
              {f.verify && f.verify.length > 0 && (
                <>
                  <div className="text-xs font-medium text-ink-2">验证步骤</div>
                  <pre className="code">{f.verify.map((s, i) => `${i + 1}. ${s}`).join('\n')}</pre>
                </>
              )}
              {f.guard && (
                <>
                  <div className="text-xs font-medium text-ink-2">防护代码</div>
                  <pre className="code">{f.guard}</pre>
                </>
              )}
            </div>
          )}
        </div>
        <div className="flex flex-col gap-1.5 shrink-0">
          {(f.fix || f.rootCause || f.guard) && (
            <button className="btn-ghost !h-7 !px-2.5 !text-xs" onClick={() => setOpen(!open)}>
              {open ? '收起' : '详情'}
            </button>
          )}
          {onFix && (
            <button className="btn !h-7 !px-2.5 !text-xs" onClick={() => onFix(f)}>修复</button>
          )}
        </div>
      </div>
    </div>
  );
}