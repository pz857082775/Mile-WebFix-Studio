import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { copyText } from '../lib/api';
import { readClipboardText, writeClipboardText } from '../lib/tauri';
import { useToast } from './Toast';

interface MenuState {
  x: number;
  y: number;
  target: HTMLElement | null;
  editable: boolean;
  readonly: boolean;
  selection: string;
  selStart: number;
  selEnd: number;
}

const TEXT_INPUT_TYPES = ['text', 'search', 'url', 'tel', 'password', 'email', 'number'];

function resolveTarget(el: HTMLElement): MenuState | null {
  const field = el.closest?.('input, textarea, [contenteditable="true"], [contenteditable=""], [role="textbox"]') as HTMLElement | null;
  if (field) {
    const tag = field.tagName.toLowerCase();
    if (tag === 'input') {
      const inp = field as HTMLInputElement;
      if (!TEXT_INPUT_TYPES.includes(inp.type)) return null;
      const s = inp.selectionStart ?? 0;
      const e = inp.selectionEnd ?? 0;
      return { x: 0, y: 0, target: inp, editable: true, readonly: inp.readOnly || inp.disabled, selection: inp.value.substring(s, e), selStart: s, selEnd: e };
    }
    if (tag === 'textarea') {
      const ta = field as HTMLTextAreaElement;
      const s = ta.selectionStart ?? 0;
      const e = ta.selectionEnd ?? 0;
      return { x: 0, y: 0, target: ta, editable: true, readonly: ta.readOnly || ta.disabled, selection: ta.value.substring(s, e), selStart: s, selEnd: e };
    }
    if (field.isContentEditable) {
      return { x: 0, y: 0, target: field, editable: true, readonly: false, selection: window.getSelection()?.toString() ?? '', selStart: 0, selEnd: 0 };
    }
  }
  const selection = window.getSelection()?.toString() ?? '';
  if (!selection) return null;
  return { x: 0, y: 0, target: null, editable: false, readonly: true, selection, selStart: 0, selEnd: 0 };
}

function Icon({ d }: { d: string }) {
  return (
    <svg viewBox="0 0 24 24" className="w-4 h-4 shrink-0" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
      <path d={d} />
    </svg>
  );
}

export function ContextMenu() {
  const [menu, setMenu] = useState<MenuState | null>(null);
  const ref = useRef<HTMLDivElement>(null);
  const toast = useToast();

  const close = useCallback(() => setMenu(null), []);

  useEffect(() => {
    const onCtx = (e: MouseEvent) => {
      e.preventDefault();
      const base = resolveTarget(e.target as HTMLElement);
      if (!base) { setMenu(null); return; }
      setMenu({ ...base, x: e.clientX, y: e.clientY });
    };
    document.addEventListener('contextmenu', onCtx);
    return () => document.removeEventListener('contextmenu', onCtx);
  }, []);

  useEffect(() => {
    if (!menu) return;
    const onDown = (e: MouseEvent) => { if (ref.current && !ref.current.contains(e.target as Node)) close(); };
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') close(); };
    const onScroll = () => close();
    document.addEventListener('mousedown', onDown);
    document.addEventListener('keydown', onKey);
    window.addEventListener('scroll', onScroll, true);
    window.addEventListener('resize', onScroll);
    window.addEventListener('blur', onScroll);
    return () => {
      document.removeEventListener('mousedown', onDown);
      document.removeEventListener('keydown', onKey);
      window.removeEventListener('scroll', onScroll, true);
      window.removeEventListener('resize', onScroll);
      window.removeEventListener('blur', onScroll);
    };
  }, [menu, close]);

  useLayoutEffect(() => {
    if (!menu || !ref.current) return;
    const r = ref.current.getBoundingClientRect();
    const x = Math.max(8, Math.min(menu.x, window.innerWidth - r.width - 8));
    const y = Math.max(8, Math.min(menu.y, window.innerHeight - r.height - 8));
    if (x !== menu.x || y !== menu.y) setMenu((m) => (m ? { ...m, x, y } : m));
  }, [menu]);

  const restore = useCallback(() => {
    if (!menu?.target) return;
    menu.target.focus();
    if (menu.target instanceof HTMLInputElement || menu.target instanceof HTMLTextAreaElement) {
      menu.target.setSelectionRange(menu.selStart, menu.selEnd);
    }
  }, [menu]);

  const doCopy = useCallback(async () => {
    if (!menu) return;
    if (menu.editable && menu.target) { restore(); document.execCommand('copy'); }
    else if (menu.selection) {
      const ok = await writeClipboardText(menu.selection);
      if (!ok) await copyText(menu.selection);
    }
    close();
  }, [menu, restore, close]);

  const doCut = useCallback(() => {
    if (!menu?.editable || menu.readonly) return;
    restore(); document.execCommand('cut'); close();
  }, [menu, restore, close]);

  const doPaste = useCallback(async () => {
    if (!menu?.editable || menu.readonly || !menu.target) return;
    let text = await readClipboardText();
    if (text == null) {
      try { text = await navigator.clipboard.readText(); }
      catch {
        restore();
        if (!document.execCommand('paste')) toast('浏览器限制读取剪贴板，请按 Ctrl+V 粘贴');
        close();
        return;
      }
    }
    if (text) { restore(); document.execCommand('insertText', false, text); }
    close();
  }, [menu, restore, close, toast]);

  const doSelectAll = useCallback(() => {
    if (!menu?.target) return;
    menu.target.focus();
    if (menu.target instanceof HTMLInputElement || menu.target instanceof HTMLTextAreaElement) {
      menu.target.select();
    } else {
      const range = document.createRange();
      range.selectNodeContents(menu.target);
      const sel = window.getSelection();
      sel?.removeAllRanges();
      sel?.addRange(range);
    }
    close();
  }, [menu, close]);

  if (!menu) return null;

  const item = 'w-full text-left px-3 h-8 text-[13px] text-ink-2 flex items-center gap-2.5 hover:bg-side hover:text-ink transition-colors disabled:opacity-40 disabled:pointer-events-none';
  const hasSel = menu.selection.length > 0;

  return (
    <div
      ref={ref}
      className="fixed card py-1.5 z-[1000] animate-popIn shadow-dialog min-w-[168px]"
      style={{ left: menu.x, top: menu.y }}
      onContextMenu={(e) => e.preventDefault()}
      onMouseDown={(e) => e.preventDefault()}
    >
      {menu.editable ? (
        <>
          <button className={item} disabled={!hasSel || menu.readonly} onClick={doCut}>
            <Icon d="M6 4a2 2 0 1 0 0 4 2 2 0 0 0 0-4Zm0 12a2 2 0 1 0 0 4 2 2 0 0 0 0-4Zm2-6 12-6M8 14l12 6" />
            剪切
          </button>
          <button className={item} disabled={!hasSel} onClick={doCopy}>
            <Icon d="M9 9h10v10H9zM5 15V5h10" />
            复制
          </button>
          <button className={item} disabled={menu.readonly} onClick={doPaste}>
            <Icon d="M9 4h6v3H9zM7 5H5v15h14V5h-2M9 12h6M9 16h4" />
            粘贴
          </button>
          <div className="h-px bg-line my-1" />
          <button className={item} onClick={doSelectAll}>
            <Icon d="M4 8V4h4M16 4h4v4M20 16v4h-4M8 20H4v-4M9 9h6v6H9z" />
            全选
          </button>
        </>
      ) : (
        <button className={item} onClick={doCopy}>
          <Icon d="M9 9h10v10H9zM5 15V5h10" />
          复制
        </button>
      )}
    </div>
  );
}