import type { PageId } from './types';

export const PAGE_ICONS: Record<PageId, string> = {
  scan: 'M12 2a10 10 0 1 0 10 10A10 10 0 0 0 12 2Zm1 15h-2v-2h2Zm0-4h-2V7h2Z',
  crawl: 'M4 4h6v6H4Zm10 0h6v6h-6ZM4 14h6v6H4Zm10 3h3m-3-3h6v6h-6Z',
  inspect: 'M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7Zm10 3a3 3 0 1 0-3-3 3 3 0 0 0 3 3Z',
  fix: 'M14.7 6.3a4.5 4.5 0 0 0-6 6L3 18l3 3 5.7-5.7a4.5 4.5 0 0 0 6-6L14 13l-3-3Z',
  kb: 'M4 19.5A2.5 2.5 0 0 1 6.5 17H20V2H6.5A2.5 2.5 0 0 0 4 4.5ZM20 17v5H6.5a2.5 2.5 0 0 1 0-5',
  ai: 'M12 2l2.4 5.3L20 8l-4 4.1.9 5.9-4.9-2.7L7.1 18l.9-5.9L4 8l5.6-.7Z',
  ext: 'M12 2l3 3-3 3-3-3Zm-7 7l3 3-3 3-3-3Zm14 0l3 3-3 3-3-3Zm-7 7l3 3-3 3-3-3Z',
  orch: 'M4 6h16M4 12h10M4 18h7m5-3l4 3-4 3',
};

export function PageIcon({ id, className }: { id: PageId; className?: string }) {
  return (
    <svg viewBox="0 0 24 24" className={className || 'w-4 h-4'} fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      <path d={PAGE_ICONS[id]} />
    </svg>
  );
}