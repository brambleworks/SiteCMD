export function reloadAppWindow() {
  if (typeof window !== "undefined") {
    window.location.reload();
  }
}
