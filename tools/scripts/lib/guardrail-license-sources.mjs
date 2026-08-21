const DIR = "apps/desktop/src-tauri/src/licensing/commands";

export const LIFECYCLE_ROOT = `${DIR}/license_lifecycle.rs`;
export const LIFECYCLE_ACTIVATION = `${DIR}/license_lifecycle_activation.rs`;
export const LIFECYCLE_VALIDATION = `${DIR}/license_lifecycle_validation.rs`;
export const LIFECYCLE_DEACTIVATION = `${DIR}/license_lifecycle_deactivation.rs`;
export const LIFECYCLE_TESTS = `${DIR}/license_lifecycle_tests.rs`;

// Production lifecycle modules in logical order.
export const LIFECYCLE_SOURCES = [
  LIFECYCLE_ROOT,
  LIFECYCLE_ACTIVATION,
  LIFECYCLE_VALIDATION,
  LIFECYCLE_DEACTIVATION,
];

/** Read all production lifecycle source in logical module order. */
export function readLifecycleSource(read) {
  return LIFECYCLE_SOURCES.map((file) => read(file)).join("\n");
}
