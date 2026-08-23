# SiteCMD Desktop UI Component Guide

This file is the contract for how the desktop app's UI is composed. Read it before adding any markup. The goal is one consistent visual system instead of subtly different recipes scattered across pages.

## Reading order

1. **[Class shape](#class-shape)** - how a className is composed
2. **[Primitives](#primitives)** - visual foundations with code snippets
3. **[Roles](#roles)** - what an object does in the product
4. **[Buttons](#buttons)** - the single source of truth for clickable actions
5. **[State conventions](#state-conventions)** - `is-*` and disabled / focus / hover
6. **[Where everything lives](#where-everything-lives)** - the source-of-truth map for CSS files
7. **[Rules](#rules)** - what to do and what not to do
8. **[Adding a new pattern](#adding-a-new-pattern)** - the process

## Class shape

Compose classes from four parts:

```tsx
className = "card card--interactive card--muted action-card is-current";
```

Canonical JSX form: `className="card card--interactive card--muted action-card is-current"`.

| Part           | Purpose                       | Example                                   |
| -------------- | ----------------------------- | ----------------------------------------- |
| Base primitive | Visual foundation             | `card`, `panel`, `tile`, `list-row`       |
| Modifier       | A variant of the base         | `card--muted`, `card--interactive`        |
| Role           | What it _does_ in the product | `action-card`, `metric-card`, `alert-row` |
| State          | Current runtime state         | `is-current`, `is-selected`, `is-active`  |

Element classes use `__`: `card__title`, `action-card__value`, `panel__header`.

## Primitives

### `card`

Standard framed content block. Use whenever you need a bordered, rounded container with default padding.

```tsx
<div className="card">
  <p>Default card content.</p>
</div>

// Variants
<div className="card card--compact">...</div>      // Tighter padding
<div className="card card--spacious">...</div>     // Roomier padding
<div className="card card--muted">...</div>        // Muted background
<div className="card card--interactive">...</div>  // Clickable, hover/focus affordances
```

Children: `.card__title`, `.card__icon`, `.card__title-rule`.

### `panel`

Larger section-level container. Use for full-width sections, tables, or grouped content above a card.

```tsx
<div className="panel">
  <div className="panel__header">Header</div>
  ...
</div>

// Variants
<div className="panel panel--muted">...</div>      // Muted background
<div className="panel panel--flush">...</div>      // No inner padding (tables, nested rows)
<div className="panel panel--large">...</div>      // Hero / callout
<div className="panel panel--empty">...</div>      // Empty-state shell
```

### `tile`

Compact dashboard or summary item. Smaller than `card`, denser than `panel`.

```tsx
<div className="tile">
  <p className="tile__label">Active Scans</p>
  <span className="tile__cta">View all</span>
</div>

// Variant
<button className="tile tile--interactive">...</button>
```

When showing a single numeric stat that needs to feel like a card (border, bg-card), use `stat-card` instead.

### `stat-card`

Stat / KPI card with `bg-card`, border, and slightly more visual weight than `tile`. The canonical class for the cards at the top of overview pages.

```tsx
<div className="stat-card">
  <p className="stat-label">Active Vulnerabilities</p>
  <span className="stat-value">0</span>
</div>
```

### `list-row`

Repeated row in a list, feed, or selection view.

```tsx
<div className="list-row">...</div>

// Variants
<button className="list-row list-row--interactive">...</button>  // Clickable
<button className="list-row list-row--action">...</button>       // Action row (settings)
<button className="list-row list-row--issue">...</button>        // Issue list row
<button className="list-row list-row--dashboard">...</button>    // Dashboard row
```

Children: `.list-row__title`, `.list-row__label`, `.list-row__chevron`.

### `fix-queue-row`

Clickable row in the FixQueue and Issues page. Includes hover background and a left-border slot for severity color.

```tsx
<button className={`fix-queue-row ${severityBorderClass(severity)}`}>...</button>
```

### `eyebrow`

Small uppercase tracked label. Use for severity, domain, scope, or status metadata inline next to titles. **Never** wrap in a pill / bg color - that violates the project's no-pills rule.

```tsx
<span className={`eyebrow ${severityToneClass(severity)}`}>{severityLabel(severity)}</span>
```

### `field-shell` and `field-control`

Form inputs. Use `field-shell` for a framed input row, `field-control` for a standalone control.

```tsx
// Framed search input
<div className="field-shell">
  <Search />
  <input className="field-shell__control" />
</div>

// Standalone control
<input className="field-control field-control--card" />
<select className="field-control field-control--select">...</select>
<textarea className="field-control field-control--muted" />
```

### `empty-state`

No-content state. Must include a direct action button - never tell the user to "go to Settings" without giving them a button to click.

```tsx
<div className="empty-state">
  <h3>No projects yet</h3>
  <p>Add your first project to start scanning.</p>
  <Button onClick={onAddProject}>Add project</Button>
</div>
```

## Roles

Roles describe what an object _does_. Combine a role with a primitive.

| Role               | Primitive        | Used for                              |
| ------------------ | ---------------- | ------------------------------------- |
| `action-card`      | `card`           | Clickable card that starts a workflow |
| `metric-card`      | `card` or `tile` | Numeric status or KPI                 |
| `project-card`     | `card`           | Project / site card                   |
| `integration-card` | `card`           | Provider connection card              |
| `alert-row`        | `list-row`       | Alert feed row                        |
| `report-preview`   | `panel`          | Report preview / mockup surface       |

```tsx
<button className="card card--interactive action-card">
  <div className="action-card__title">Run scan</div>
  <div className="action-card__detail">Start a Full scan</div>
</button>
```

## Typography

Use these classes for every piece of text. Tailwind is removed, so there is no `text-[XXpx]` utility to fall back on - a size lives either in this scale or in a component's own class, never inline in the JSX.

### The scale

| Class              | Size      | Color            | Used for                  |
| ------------------ | --------- | ---------------- | ------------------------- |
| `.text-body`       | 13px      | foreground       | standard paragraph        |
| `.text-body-muted` | 12px      | muted-foreground | descriptions, helper text |
| `.text-meta`       | 11px      | muted-foreground | metadata, secondary text  |
| `.text-meta-bold`  | 11px bold | muted-foreground | callouts, mini-buttons    |
| `.text-micro`      | 10px      | muted-foreground | timestamps, fine print    |

### Modifiers

| Class            | What it adds                   |
| ---------------- | ------------------------------ |
| `.text-strong`   | bold + foreground color        |
| `.text-relaxed`  | leading-relaxed for multi-line |
| `.text-truncate` | single-line clamp              |

### Numeric display roles

For metric values, deltas, and big numeric displays. Pre-includes `tabular-nums` so columns of numbers align.

| Class             | Size + weight                | Used for                       |
| ----------------- | ---------------------------- | ------------------------------ |
| `.metric-display` | 40px font-black leading-none | huge score / total display     |
| `.metric-value`   | 22px font-black              | KPI / delta value              |
| `.metric-delta`   | 13px bold                    | change indicator next to value |
| `.metric-mini`    | 12px bold                    | small numeric callout          |

### Mono text

| Class               | Size + color             | Used for                |
| ------------------- | ------------------------ | ----------------------- |
| `.text-mono-sm`     | 12px mono foreground     | inline code refs        |
| `.text-mono-xs`     | 11px mono muted          | small mono labels       |
| `.mono-value-block` | 12px mono with break-all | dossier evidence values |

### Eyebrow labels

Already documented under [Primitives](#primitives) but listing here for completeness:

| Class                | What it does                                                 |
| -------------------- | ------------------------------------------------------------ |
| `.eyebrow`           | 10px uppercase tracking-0.16em font-bold (color from caller) |
| `.eyebrow--alt`      | 11px uppercase tracking-0.15em font-black variant            |
| `.section-label`     | 10px muted uppercase tracking-0.16em font-bold               |
| `.section-label-lg`  | 11px muted uppercase tracking-0.2em font-bold                |
| `.section-label-mid` | 11px muted uppercase tracking-0.15em font-bold               |

### Migration examples

```tsx
// Before
<p className="text-[11px] text-muted-foreground">Last scan</p>
// After
<p className="text-meta">Last scan</p>

// Before
<span className={`text-[22px] font-black tabular-nums ${up ? "text-emerald-400" : "text-red-400"}`}>{delta}</span>
// After
<span className={`metric-value ${up ? "text-emerald-400" : "text-red-400"}`}>{delta}</span>

// Before
<h3 className="text-[13px] font-bold text-foreground">Title</h3>
// After
<h3 className="text-body text-strong">Title</h3>
```

## Spacing

Tailwind has been removed. Spacing is driven by the `--space-*` token scale (defined once in `tokens.css`) and applied through **semantic classes** - there is no utility layer, so there are no `mt-2`/`gap-3`/`space-y-4` classes to reach for.

### The scale

`--space-*` tokens (rem, keyed to a 16px root), used throughout the CSS:

| Token             | rem      | px   | Common use                           |
| ----------------- | -------- | ---- | ------------------------------------ |
| `--space-hair`    | 0.125rem | 2px  | hairline gaps inside dense rows      |
| `--space-tight`   | 0.25rem  | 4px  | tight gaps                           |
| `--space-snug`    | 0.5rem   | 8px  | default gap inside a card            |
| `--space-base`    | 0.75rem  | 12px | section internal gap, common padding |
| `--space-card`    | 1rem     | 16px | card / panel padding                 |
| `--space-section` | 1.5rem   | 24px | section breaks                       |
| `--space-hero`    | 2rem     | 32px | hero spacing                         |

Off-scale values (6px, 10px, 20px...) snap to the nearest token, or are written as a literal `rem` **only** for a component's intrinsic dimension (a button height, an icon size) - never for layout rhythm.

### Where padding lives

Padding is **role-owned**. A `.card`, `.panel`, `.list-row`, etc. defines its own internal padding via `--space-*`. Don't add padding on top of a card; use a modifier (`card--compact`, `card--spacious`) when you need a non-default.

### Vertical rhythm: `.stack-*`

Spacing between stacked siblings uses the owl-margin `.stack-*` primitives (layout.css), keyed to the token scale:

`.stack-hair` (2) · `.stack-tight` (4) · `.stack-snug` (8) · `.stack-base` (12) · `.stack-card` (16) · `.stack-section` (24) · `.stack-hero` (32)

```tsx
<div className="stack-snug">
  <h3 className="text-body text-strong">Title</h3>
  <p className="text-body-muted">Description</p>
</div>
```

### Horizontal rows: `.row-*`

Flex rows use the `.row` family (data.css): `.row` (items-center, 8px gap), `.row-tight` (4), `.row-loose` (12), `.row-wrap`, `.row-between`, `.row-between-top`, `.row-start`, `.row-end`, `.row-actions`. `.flex-fill` (flex:1 + min-width:0), `.no-shrink`, and `.min-w-0` cover the common flex-child needs.

### One-off spacing

When a specific element needs a specific margin, add it to that element's own named class (e.g. `.deploy-range-label { margin-top: var(--space-tight); }`), referencing a `--space-*` token. Never inline it in the JSX.

### Banned

- **Any** inline spacing utility (`mt-2`, `gap-3`, `space-y-4`, `p-4`) - the utility layer is gone.
- Raw `px`/`rem` spacing that doesn't reference a `--space-*` token, except a component's intrinsic dimensions.
- Adding redundant padding on top of a role primitive.
- Custom-pixel offsets like `-bottom-[9px]` unless you're building a visual indicator that genuinely can't fit the scale.

### Typography migration

The inline `text-[Xpx]` migration is complete. The repository guardrail rejects any new arbitrary pixel text size, so use the typography scale or a component-owned class.

## Buttons

Use the `<Button>` component for every clickable action. It emits short, predictable classes:

```html
<button class="btn btn--default btn--sm">Run Scan</button>
```

```tsx
<Button>Default primary button</Button>
<Button variant="secondary">Secondary</Button>
<Button variant="outline">Outline</Button>
<Button variant="ghost">Ghost</Button>
<Button variant="destructive">Delete</Button>
<Button variant="link">Read more</Button>
<Button variant="accent">Accent</Button>

<Button size="sm">Small</Button>
<Button size="lg">Large</Button>
<Button size="icon" aria-label="Settings"><Settings /></Button>
```

For clickable rows, tiles, and other elements that need button semantics but not button chrome:

```tsx
<Button unstyled className="action-card card card--interactive" onClick={...}>
  ...
</Button>
```

The button's visual spec lives entirely in `buttons.css`. Adding a new variant means adding a class there and adding it to `ButtonVariant` in `button.tsx`. Never inline brand-button colors in a regular className.

## State conventions

State is communicated with `is-*` classes that pair with a base:

```tsx
<button className="nav-item is-active">Dashboard</button>
<div className="list-row is-selected">...</div>
<button className="tile tile--interactive is-disabled" aria-disabled="true">...</button>
```

Standard state classes: `is-active`, `is-selected`, `is-current`, `is-disabled`, `is-loading`.

For native form / link disabled states, prefer the native `disabled` / `aria-disabled` attribute - primitives like `.btn` and `.list-row--action` already style `:disabled`.

## Where everything lives

```
src/styles/
├── tokens.css          ← CSS variables (colors, radii, theme)
├── base.css            ← html/body, resets
├── typography.css      ← .headline-*, .body-text, .mono-*
├── layout.css          ← .row-between, .grid-*, .stack-*, .panel
├── chrome.css          ← .app-topbar, .nav-*
├── cards.css           ← .card, .tile, .panel
├── data.css            ← .list-row, .fix-queue-row, .eyebrow, .stat-card
├── interactive.css     ← .icon-btn, .segmented-control, .toggle-btn, .tab
├── buttons.css         ← .btn and all variants/sizes
├── dossier.css         ← Dossier-specific layout
├── utilities.css       ← Last-resort utilities
├── animations.css      ← Keyframes
└── pages/              ← Page-specific layouts
    ├── finance.css
    ├── settings.css
    └── alerts.css
```

When in doubt about where a new pattern goes: if it's _visual_ and likely reused, it goes in `cards.css` / `data.css` / `interactive.css`. If it's _page-specific_, it goes under `pages/`.

## Rules

**Required**

1. Use the `<Button>` component for every clickable action. Never write a manual `<button>` with inline `bg-*` or `hover:*` classes.
2. Move any long, multi-class `className` into a named class. Every class must resolve to a real rule in `styles/`: the Tailwind-removal guardrail fails CI on any utility-shaped class (`mt-2`, `bg-muted/40`, `text-indigo-600`) that has no backing CSS, since there is no Tailwind engine left to generate it.
3. If a visual pattern appears twice, add or reuse a named class.
4. Use existing primitives + roles before adding new ones. Check this guide first.
5. Class names describe the _role_, not the visual recipe. Good: `integration-card`. Bad: `card-with-shadow-and-hover`.

**Banned**

1. Inline `style={{ ... }}` attributes (the escape hatches are `progress-bar`'s runtime width, `score-ring`'s size-derived geometry, and the react-pdf `ReportPDFSections`).
2. Rounded pill badges with background colors for severity / scope / status metadata. Use `.eyebrow` + a color class instead.
3. New Tailwind utility classes with no backing CSS. Tailwind was removed root and branch; a utility only works if `styles/` hand-writes it (the color utilities plus a small set like `flex` / `min-w-0` do). Anything else - `mt-2`, `bg-muted/40`, `text-indigo-600` - is dead, and the Tailwind-removal guardrail fails CI on it. Add a semantic class instead.
4. CVA (`class-variance-authority`). The Button refactor removed it; do not reintroduce.
5. Hardcoded hex colors. Use CSS variables from `tokens.css`.
6. Hover effects on non-clickable elements. Hover means _this responds to clicks_.

## Adding a new pattern

1. Confirm it isn't already covered. Check this guide and grep `styles/*.css` for similar names.
2. Decide the right primitive base (`card`, `panel`, `tile`, `list-row`, `eyebrow`).
3. Pick a _role_ name that describes the product job, not the visual recipe.
4. Add the class to the right CSS file (see [Where everything lives](#where-everything-lives)).
5. Add an entry to this guide if it's a primitive or a role likely to be reused.
6. Use the new class - never paste utility soup into the consuming component.

If you find yourself wanting to do something this guide doesn't cover, edit this file rather than improvising. Future-you and the AI agents reading the codebase will thank you.
