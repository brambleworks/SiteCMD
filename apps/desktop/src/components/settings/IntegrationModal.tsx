import type { ReactNode } from "react";
import { X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";

interface IntegrationModalProps {
  title: string;
  icon: ReactNode;
  onClose: () => void;
  children: ReactNode;
}

export function IntegrationModal({ title, icon, onClose, children }: IntegrationModalProps) {
  return (
    <Dialog
      label={title}
      onClose={onClose}
      backdropClassName="dialog--soft"
      className="modal-card modal-card--scroll">
      <div className="row-loose">
        {icon}
        <p className="row-title-lg flex-fill text-truncate">{title}</p>
        <Button unstyled type="button" aria-label="Close" onClick={onClose} className="icon-btn">
          <X className="icon-md" aria-hidden="true" />
        </Button>
      </div>
      {children}
    </Dialog>
  );
}
