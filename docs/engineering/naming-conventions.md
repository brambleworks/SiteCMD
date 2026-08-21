# Naming Conventions

Names should make a file's role obvious before you open it. Avoid mixed
`camelCase`, `kebab-case`, and `PascalCase` for the same kind of file.

## Frontend Files

- Use `PascalCase.tsx` for feature React components and component-bearing
  section modules.
- Use `usePascalCase.ts` or `usePascalCase.tsx` only when the file's primary
  export is a React hook with the same name.
- Use `kebab-case.ts` for non-component modules such as models, loaders,
  helpers, command builders, reducers, and presentation utilities.
- Single-word lowercase utility files are allowed when they are unambiguous,
  for example `types.ts`, `utils.ts`, `store.ts`, and `tokens.ts`.
- Test files should mirror the source filename and may add a descriptive test
  suffix such as `.behavior.test.tsx`, `.render.test.tsx`, or
  `.performance.test.tsx`.

## Intentional Exceptions

- `apps/desktop/src/components/ui` follows package-style primitive naming, so lowercase and
  kebab-case primitive files such as `button.tsx`, `card.tsx`, and
  `surface-state.tsx` are expected there. Shared component modules in this
  directory may still use `PascalCase.tsx` when the file primarily exports that
  component.
- Rust backend files keep Rust's `snake_case.rs` convention.
- Public Tauri command names, serialized fields, and persisted data keys should
  not be renamed just to match frontend filename rules.

## Examples

- Component: `ActionItemsCard.tsx`
- Hook: `useDashboardData.ts`
- Model/helper: `dashboard-data-state.ts`
- Barrel/helper module: `update-sections.ts`
- UI primitive exception: `components/ui/button.tsx`

## Checks

Run the direct naming audit when a rename feels suspicious or when an agent has
split files across multiple areas:

```sh
pnpm naming:audit
```

This is intentionally more explainable than the ESLint filename globs. ESLint
still enforces the convention during normal lint runs, and the audit script gives
humans a quick, focused way to see exactly which file does not match the
documented role.

When extracting code, name the file for the thing it owns. Do not create vague
files like `useSomethingState.ts` unless the primary export is actually the
hook named `useSomethingState`, and do not add new camelCase helper filenames.
