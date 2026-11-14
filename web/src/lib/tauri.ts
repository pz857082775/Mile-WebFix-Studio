export interface TauriWindow {
  minimize(): Promise<void>;
  toggleMaximize(): Promise<void>;
  close(): Promise<void>;
  isMaximized(): Promise<boolean>;
  onResized(cb: () => void): Promise<() => void>;
}

export function tauriWindow(): TauriWindow | null {
  const t = (window as unknown as { __TAURI__?: { window?: { getCurrentWindow?: () => TauriWindow } } }).__TAURI__;
  if (!t?.window?.getCurrentWindow) return null;
  return t.window.getCurrentWindow();
}

// 订阅桌面壳事件（如托盘「设置软件」→ wf-navigate）;网页版无 __TAURI__ 时返回 null
export function onTauriEvent(name: string, cb: (payload: unknown) => void): (() => void) | null {
  const t = (window as unknown as {
    __TAURI__?: { event?: { listen?: (n: string, h: (e: { payload: unknown }) => void) => Promise<() => void> } };
  }).__TAURI__;
  if (!t?.event?.listen) return null;
  let un: (() => void) | undefined;
  t.event.listen(name, (e) => cb(e.payload)).then((u) => { un = u; }).catch(() => {});
  return () => { un?.(); };
}