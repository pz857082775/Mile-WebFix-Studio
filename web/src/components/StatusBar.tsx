import { PAGE_META } from '../lib/format';
import type { PageId } from '../lib/types';

export function StatusBar({ active, tabCount, ruleCount, onChangelog }: { active: PageId | null; tabCount: number; ruleCount: number; onChangelog: () => void }) {
  return (
    <footer className="statusbar">
      <span className="font-medium text-ink-2">{active ? PAGE_META[active][0] : '首页'}</span>
      <span>{tabCount} 个页签</span>
      <span className="flex-1" />
      <span className="inline-flex items-center gap-1.5">
        <span className="w-1.5 h-1.5 rounded-full bg-ok" />
        服务运行中 · 端口 {location.port || '80'}
      </span>
      <span className="w-px h-3 bg-line" />
      <span>规则 {ruleCount} 条</span>
      <span className="w-px h-3 bg-line" />
      <span>© 米乐 · <button className="hover:text-accent cursor-pointer" title="查看历史更新记录" onClick={onChangelog}>v1.2.1</button> · 微信 mileai8</span>
    </footer>
  );
}