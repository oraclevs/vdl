import { createContext, useCallback, useContext, useRef, useState } from 'react';

const ToastContext = createContext(null);

const TONES = {
  info: 'border-line bg-card text-ink',
  success: 'border-teal/30 bg-teal-soft text-teal',
  error: 'border-rose/30 bg-rose-soft text-rose',
  warn: 'border-coral/30 bg-coral-soft text-coral',
};

export function ToastProvider({ children }) {
  const [toasts, setToasts] = useState([]);
  const counter = useRef(0);

  const push = useCallback((message, tone = 'info') => {
    const id = ++counter.current;
    setToasts((prev) => [...prev, { id, message, tone }]);
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 4400);
  }, []);

  return (
    <ToastContext.Provider value={push}>
      {children}
      <div className="fixed bottom-6 right-6 z-50 flex flex-col gap-2.5 max-w-[22rem]">
        {toasts.map((t) => (
          <div
            key={t.id}
            className={`animate-rise rounded-2xl border px-4 py-3 text-sm shadow-lg shadow-ink/5 ${TONES[t.tone] || TONES.info}`}
          >
            {t.message}
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}

export function useToast() {
  const ctx = useContext(ToastContext);
  if (!ctx) throw new Error('useToast must be used within a ToastProvider');
  return ctx;
}
