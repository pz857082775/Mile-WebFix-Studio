import { Modal } from './Modal';
import { CHANGELOG, CHANGELOG_NOTE } from '../lib/changelog';

export function Changelog({ onClose }: { onClose: () => void }) {
  return (
    <Modal title="历史更新记录" onClose={onClose} footer={<button className="btn" onClick={onClose}>关闭</button>}>
      <div className="space-y-3">
        {CHANGELOG.map((e) => (
          <div key={e.version} className="rounded-lg border border-line bg-side px-4 py-3">
            <div className="flex items-center gap-2">
              <span className="text-[14px] font-bold text-ink">v{e.version}</span>
              {e.tag && <span className="badge cat">{e.tag}</span>}
              <span className="text-[11px] text-ink-3 ml-auto">{e.date}</span>
            </div>
            <ul className="mt-2 space-y-1 text-xs text-ink-2 leading-relaxed list-disc pl-4">
              {e.items.map((it, i) => <li key={i}>{it}</li>)}
            </ul>
          </div>
        ))}
      </div>
      <p className="text-[11px] text-ink-3 mt-3">{CHANGELOG_NOTE}</p>
    </Modal>
  );
}