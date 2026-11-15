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

// 调用桌面壳命令（如 reveal_in_folder）；网页版无 __TAURI__ 时返回 null
export function tauriInvoke<T = unknown>(cmd: string, args?: Record<string, unknown>): Promise<T> | null {
  const t = (window as unknown as {
    __TAURI__?: { core?: { invoke?: (c: string, a?: Record<string, unknown>) => Promise<T> } };
  }).__TAURI__;
  if (!t?.core?.invoke) return null;
  return t.core.invoke(cmd, args);
}

// 在系统文件管理器中打开更新包所在文件夹（桌面版）；网页版返回 false 由前端降级为复制路径
export async function revealInFolder(path: string): Promise<boolean> {
  const p = tauriInvoke('reveal_in_folder', { path });
  if (!p) return false;
  try { await p; return true; } catch { return false; }
}