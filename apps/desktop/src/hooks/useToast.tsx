import { useState, useCallback, useMemo, createContext, useContext, useRef } from "react";
import { CheckCircle, XCircle, AlertTriangle, Info, X } from "lucide-react";
import { Button } from "@/components/ui/button";

type ToastType = "success" | "error" | "warning" | "info";

interface Toast {
  id: number;
  type: ToastType;
  title: string;
  description?: string;
  exiting?: boolean;
}

interface ToastContextValue {
  toast: (type: ToastType, title: string, description?: string) => void;
  success: (title: string, description?: string) => void;
  error: (title: string, description?: string) => void;
  warning: (title: string, description?: string) => void;
  info: (title: string, description?: string) => void;
}

const ToastContext = createContext<ToastContextValue>({
  toast: () => {},
  success: () => {},
  error: () => {},
  warning: () => {},
  info: () => {},
});

export function useToast() {
  return useContext(ToastContext);
}

let nextId = 0;

export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const timersRef = useRef<Map<number, ReturnType<typeof setTimeout>>>(new Map());

  const dismiss = useCallback((id: number) => {
    const timer = timersRef.current.get(id);
    if (timer) {
      clearTimeout(timer);
      timersRef.current.delete(id);
    }
    setToasts((prev) => prev.map((t) => (t.id === id ? { ...t, exiting: true } : t)));
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 200);
  }, []);

  const addToast = useCallback(
    (type: ToastType, title: string, description?: string) => {
      const id = nextId++;
      setToasts((prev) => [...prev, { id, type, title, description }]);

      const duration = type === "error" ? 6000 : 4000;
      const timer = setTimeout(() => dismiss(id), duration);
      timersRef.current.set(id, timer);
    },
    [dismiss],
  );

  const value = useMemo<ToastContextValue>(
    () => ({
      toast: addToast,
      success: (t: string, d?: string) => addToast("success", t, d),
      error: (t: string, d?: string) => addToast("error", t, d),
      warning: (t: string, d?: string) => addToast("warning", t, d),
      info: (t: string, d?: string) => addToast("info", t, d),
    }),
    [addToast],
  );

  return (
    <ToastContext.Provider value={value}>
      {children}
      <div className="toast-stack" role="status" aria-live="polite">
        {toasts.map((t) => (
          <ToastItem key={t.id} toast={t} onDismiss={() => dismiss(t.id)} />
        ))}
      </div>
    </ToastContext.Provider>
  );
}

const ICONS: Record<ToastType, React.ReactNode> = {
  success: <CheckCircle className="icon-md text-score-excellent" />,
  error: <XCircle className="icon-md text-severity-critical" />,
  warning: <AlertTriangle className="icon-md text-severity-medium" />,
  info: <Info className="icon-md text-brand" />,
};

const BORDERS: Record<ToastType, string> = {
  success: "toast-item--success",
  error: "toast-item--error",
  warning: "toast-item--warning",
  info: "toast-item--info",
};

function ToastItem({ toast, onDismiss }: { toast: Toast; onDismiss: () => void }) {
  return (
    <div
      className={`toast-item ${BORDERS[toast.type]} ${
        toast.exiting ? "toast-item--exiting" : "animate-in slide-in-from-right-5"
      }`}>
      <div className="toast-icon">{ICONS[toast.type]}</div>
      <div className="flex-fill">
        <p className="toast-title">{toast.title}</p>
        {toast.description && <p className="muted-text toast-desc">{toast.description}</p>}
      </div>
      <Button
        unstyled
        type="button"
        onClick={onDismiss}
        className="text-hover toast-dismiss"
        aria-label="Dismiss notification">
        <X className="icon-sm" aria-hidden="true" />
      </Button>
    </div>
  );
}
