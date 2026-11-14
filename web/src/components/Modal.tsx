import { useEffect, type ReactNode } from 'react';

export function Modal({ title, onClose, children, wide, small, footer }: {
  title: string;
  onClose: () => void;
  children: ReactNode;
  wide?: boolean;
  small?: boolean;
  footer?: ReactNode;
}) {
  useEffect(() => {
    const h = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose(); };
    window.addEventListener('keydown', h);
    return () => window.removeEventListener('keydown', h);
  }, [onClose]);

  const width = wide ? 'w-[860px]' : small ? 'w-[400px]' : 'w-[560px]';
  return (
    <div className="fixed inset-0 z-[999] flex items-center justify-center p-6" onClick={onClose}>
      <div className="absolute inset-0 bg-[rgba(29,33,41,.40)] animate-fadeIn" />
      <div
        role="dialog" aria-modal="true" aria-label={title}
        className={`relative bg-white rounded-[10px] border border-line shadow-dialog ${width} max-w-full max-h-[86vh] flex flex-col animate-dlgIn`}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="dlg-title rounded-t-[10px]">
          <span className="w-5 h-5 rounded bg-accent-soft text-accent flex items-center justify-center shrink-0">
            <svg viewBox="0 0 24 24" className="w-3 h-3" fill="none" stroke="currentColor" strokeWidth="2"><path d="M12 8v5m0 3.5v.5M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20Z" /></svg>
          </span>
          <h3 className="flex-1 min-w-0 text-[13px] font-semibold text-ink truncate">{title}</h3>
          <button className="icon-btn !w-7 !h-7 hover:!bg-[#ffece8] hover:!text-danger" onClick={onClose} aria-label="关闭">
            <svg viewBox="0 0 24 24" className="w-3.5 h-3.5" fill="none" stroke="currentColor" strokeWidth="2"><path d="M6 6l12 12M18 6L6 18" /></svg>
          </button>
        </div>
        <div className="p-5 overflow-auto">{children}</div>
        {footer && <div className="dlg-footer rounded-b-[10px]">{footer}</div>}
      </div>
    </div>
  );
}

export function Confirm({ title, message, okText = '确定', danger, onOk, onClose }: {
  title: string;
  message: string;
  okText?: string;
  danger?: boolean;
  onOk: () => void;
  onClose: () => void;
}) {
  return (
    <Modal
      title={title} onClose={onClose} small
      footer={<>
        <button className="btn-ghost" onClick={onClose}>取消</button>
        <button className={danger ? 'btn-danger-solid' : 'btn'} onClick={() => { onOk(); onClose(); }}>{okText}</button>
      </>}
    >
      <div className="flex items-start gap-3">
        <span className={`w-9 h-9 rounded-full shrink-0 flex items-center justify-center ${danger ? 'bg-[#ffece8] text-danger' : 'bg-accent-soft text-accent'}`}>
          <svg viewBox="0 0 24 24" className="w-[18px] h-[18px]" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            {danger
              ? <path d="M12 3 2.5 20h19L12 3Zm0 7v4m0 3v.5" />
              : <path d="M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20Zm-1.5 7.5a1.5 1.5 0 1 1 2.6 1c-.6.6-1.1 1-1.1 2m0 3v.5" />}
          </svg>
        </span>
        <p className="text-sm text-ink-2 leading-relaxed pt-1.5">{message}</p>
      </div>
    </Modal>
  );
}