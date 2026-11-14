import type { ReactNode } from 'react';
import { PAGE_META } from '../lib/format';
import type { PageId } from '../lib/types';

export function PageHeader({ id, children }: { id: PageId; children?: ReactNode }) {
  return (
    <div className="flex items-center gap-3 mb-4 animate-paneIn">
      <div className="flex-1 min-w-0">
        <div className="text-[17px] font-bold text-ink leading-tight">{PAGE_META[id][0]}</div>
        <div className="text-xs text-ink-3 mt-1 truncate">{PAGE_META[id][1]}</div>
      </div>
      {children}
    </div>
  );
}