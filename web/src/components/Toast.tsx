import { createContext, useCallback, useContext, useRef, useState, type ReactNode } from 'react';

interface ToastItem { id: number; msg: string }
const ToastCtx = createContext<(msg: string) => void>(() => {});
export const useToast = () => useContext(ToastCtx);

export function ToastProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<ToastItem[]>([]);
  const seq = useRef(0);
  const toast = useCallback((msg: string) => {
    const id = ++seq.current;
    setItems((s) => [...s, { id, msg }]);
    setTimeout(() => setItems((s) => s.filter((t) => t.id !== id)), 3200);
  }, []);
  return (
    <ToastCtx.Provider value={toast}>
      {children}
      <div className="fixed bottom-6 left-1/2 -translate-x-1/2 z-[9999] flex flex-col items-center gap-2 pointer-events-none">
        {items.map((t) => (
          <div key={t.id} className="animate-popIn px-4 py-2.5 rounded-pane bg-ink text-white text-sm shadow-pane max-w-[70vw]">
            {t.msg}
          </div>
        ))}
      </div>
    </ToastCtx.Provider>
  );
}