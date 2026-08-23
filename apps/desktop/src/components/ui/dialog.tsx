import {
  useEffect,
  useRef,
  type KeyboardEvent,
  type MouseEvent,
  type ReactNode,
  type SyntheticEvent,
} from "react";
import { createPortal } from "react-dom";
import { cn } from "@/lib/utils";

interface DialogProps {
  /** Accessible name; pass `labelledBy` instead when a heading names the dialog. */
  label?: string;
  labelledBy?: string;
  describedBy?: string;
  onClose: () => void;
  /** Clicking the backdrop closes the dialog. Handoffs and running scans pass false. */
  dismissOnBackdrop?: boolean;
  /** Escape closes the dialog. A running scan passes false. */
  closeOnEscape?: boolean;
  /** Classes for the centered panel, for example `modal-card modal-card--scroll`. */
  className?: string;
  /** Classes for the full-screen dialog element, for example `dialog--soft`. */
  backdropClassName?: string;
  children: ReactNode;
}

/** Modal on the native element: top layer, focus trap, inert page, Escape. */
export function Dialog({
  label,
  labelledBy,
  describedBy,
  onClose,
  dismissOnBackdrop = true,
  closeOnEscape = true,
  className,
  backdropClassName,
  children,
}: DialogProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const onCloseRef = useRef(onClose);
  useEffect(() => {
    onCloseRef.current = onClose;
  }, [onClose]);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    if (!dialog.open) dialog.showModal();
    return () => {
      if (dialog.open) dialog.close();
      opener?.focus();
    };
  }, []);

  // The browser's close request (Escape, Android back) must not bypass React state.
  const handleCancel = (event: SyntheticEvent<HTMLDialogElement>) => {
    event.preventDefault();
    if (closeOnEscape) onCloseRef.current();
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLDialogElement>) => {
    if (event.key !== "Escape") return;
    event.preventDefault();
    if (closeOnEscape) onCloseRef.current();
  };

  const handleClick = (event: MouseEvent<HTMLDialogElement>) => {
    if (dismissOnBackdrop && event.target === event.currentTarget) onCloseRef.current();
  };

  return createPortal(
    <dialog
      ref={dialogRef}
      className={cn("dialog", backdropClassName)}
      aria-label={label}
      aria-labelledby={labelledBy}
      aria-describedby={describedBy}
      onCancel={handleCancel}
      onKeyDown={handleKeyDown}
      onClick={handleClick}>
      <div className={cn("dialog-panel", className)}>{children}</div>
    </dialog>,
    document.body,
  );
}
