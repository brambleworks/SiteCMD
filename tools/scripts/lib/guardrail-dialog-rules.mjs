const DIALOG_PRIMITIVE = "apps/desktop/src/components/ui/dialog.tsx";
// Hand-rolled modals still waiting to move onto the Dialog primitive. Lower only.
const HAND_ROLLED_DIALOG_BUDGET = 3;
const ROLE_DIALOG_RE = /role="dialog"/g;

export function handRolledDialogFailures(read, sourceFiles) {
  const offenders = [];
  let total = 0;
  for (const file of sourceFiles) {
    if (!file.endsWith(".tsx") || /\.(test|spec)\.tsx$/.test(file)) continue;
    if (file === DIALOG_PRIMITIVE) continue;
    const count = (read(file).match(ROLE_DIALOG_RE) ?? []).length;
    if (count === 0) continue;
    total += count;
    offenders.push(`${file} (${count})`);
  }
  if (total <= HAND_ROLLED_DIALOG_BUDGET) return [];
  return [
    `Hand-rolled role="dialog" count regressed: ${total} (budget ${HAND_ROLLED_DIALOG_BUDGET}). Render modals through components/ui/dialog.tsx, which gives the top layer, a focus trap, an inert page, and Escape for free: ${offenders.join(", ")}`,
  ];
}
