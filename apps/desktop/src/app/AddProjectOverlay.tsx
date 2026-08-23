import { AddProjectForm } from "@/app/lazy-pages";
import { Dialog } from "@/components/ui/dialog";
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
  return (
    <Dialog
      label="Add project"
      onClose={onCancel}
      backdropClassName="dialog--top"
      className="add-project-panel">
      <AddProjectForm onCreated={onCreated} onCancel={onCancel} onNavigate={onNavigate} />
    </Dialog>
  );
}
