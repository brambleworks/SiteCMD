import { useEffect } from "react";
import { AddProjectForm } from "@/app/lazy-pages";
import type { NavTarget } from "@/components/layout/nav-page";

/** Modal overlay wrapper for AddProjectForm. */
export function AddProjectOverlay({
  onCreated,
  onCancel,
  onNavigate,
}: {
  onCreated: (projectId: number) => void;
  onCancel: () => void;
  onNavigate?: (page: NavTarget) => void;
}) {
  useEscapeKey(onCancel);
  return (
    <div
      className="overlay-backdrop overlay-backdrop--add-project"
      onClick={onCancel}
      role="dialog"
      aria-modal="true"
      aria-label="Add project">
      <div className="add-project-panel" onClick={(e) => e.stopPropagation()}>
        <AddProjectForm onCreated={onCreated} onCancel={onCancel} onNavigate={onNavigate} />
      </div>
    </div>
  );
}

/** Close a modal on Escape key. Attaches/detaches a keydown listener. */
function useEscapeKey(onClose: () => void) {
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [onClose]);
}
