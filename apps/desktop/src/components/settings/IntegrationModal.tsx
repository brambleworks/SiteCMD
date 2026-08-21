import { useEffect, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { X } from "lucide-react";

import { Button } from "@/components/ui/button";

interface IntegrationModalProps {
  title: string;
  icon: ReactNode;
  onClose: () => void;
  children: ReactNode;
}

export function IntegrationModal({ title, icon, onClose, children }: IntegrationModalProps) {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      onClose();
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [onClose]);

  return createPortal(
    <div className="overlay-backdrop overlay-backdrop--soft" onClick={onClose}>
      <div
        className="modal-card modal-card--scroll"
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onClick={(event) => event.stopPropagation()}>
        <div className="row-loose">
          {icon}
          <p className="row-title-lg flex-fill text-truncate">{title}</p>
          <Button unstyled type="button" aria-label="Close" onClick={onClose} className="icon-btn">
            <X className="icon-md" aria-hidden="true" />
          </Button>
        </div>
        {children}
      </div>
    </div>,
    document.body,
  );
}
