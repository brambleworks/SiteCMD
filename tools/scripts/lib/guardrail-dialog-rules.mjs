const DIALOG_PRIMITIVE = "apps/desktop/src/components/ui/dialog.tsx";
// Hand-rolled modals still waiting to move onto the Dialog primitive. Lower only.
const HAND_ROLLED_DIALOG_BUDGET = 0;
const ROLE_DIALOG_RE = /role="dialog"/g;
// Also count aria-modal on its own (a shell can carry it without role="dialog")
// and a raw <dialog> tag reached without the shared primitive.
const ARIA_MODAL_RE = /aria-modal=/g;
const RAW_DIALOG_TAG_RE = /<dialog\b/g;

function countMatches(source, pattern) {
  return (source.match(pattern) ?? []).length;
}

export function handRolledDialogFailures(read, sourceFiles) {
  const offenders = [];
  let total = 0;
  for (const file of sourceFiles) {
    if (!file.endsWith(".tsx") || /\.(test|spec)\.tsx$/.test(file)) continue;
    if (file === DIALOG_PRIMITIVE) continue;
    const source = read(file);
    const count =
      countMatches(source, ROLE_DIALOG_RE) +
      countMatches(source, ARIA_MODAL_RE) +
      countMatches(source, RAW_DIALOG_TAG_RE);
    if (count === 0) continue;
    total += count;
    offenders.push(`${file} (${count})`);
  }
  if (total <= HAND_ROLLED_DIALOG_BUDGET) return [];
  return [
    `Hand-rolled role="dialog" count regressed: ${total} (budget ${HAND_ROLLED_DIALOG_BUDGET}). Render modals through components/ui/dialog.tsx, which gives the top layer, a focus trap, an inert page, and Escape for free: ${offenders.join(", ")}`,
  ];
}
