---
name: SiteCMD Desktop
description: A dark-first instrument console for site and code health - dense, tonal, signal-driven
colors:
  brand-blue: "oklch(0.67 0.214 259.13)"
  brand-blue-press: "oklch(0.47 0.214 259.13)"
  signal-amber: "oklch(0.769 0.188 70.08)"
  surface-floor: "#09090b"
  surface-low: "#0e0e10"
  surface-raised: "#131315"
  surface-container: "#1c1b1d"
  surface-high: "#2a2a2c"
  surface-highest: "#353437"
  ink: "#e5e1e4"
  ink-muted: "#b0b3c0"
  ghost-border: "rgba(255, 255, 255, 0.08)"
  obsidian-glass: "rgba(20, 20, 22, 0.85)"
  score-excellent: "#34d399"
  score-good: "#fbbf24"
  score-attention: "#fb923c"
  score-poor: "#f87171"
  score-critical: "#ef4444"
  severity-high: "#f97316"
  severity-medium: "#eab308"
  severity-low: "#60a5fa"
typography:
  display:
    fontFamily: "Inter, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif"
    fontSize: "40px"
    fontWeight: 900
    lineHeight: 1
    letterSpacing: "-0.01em"
  headline:
    fontFamily: "Inter, -apple-system, BlinkMacSystemFont, sans-serif"
    fontSize: "22px"
    fontWeight: 900
    lineHeight: 1
  title:
    fontFamily: "Inter, -apple-system, BlinkMacSystemFont, sans-serif"
    fontSize: "14px"
    fontWeight: 600
    lineHeight: 1.2
    letterSpacing: "0.02em"
  body:
    fontFamily: "Inter, -apple-system, BlinkMacSystemFont, sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.6
  label:
    fontFamily: "Inter, -apple-system, BlinkMacSystemFont, sans-serif"
    fontSize: "10px"
    fontWeight: 700
    lineHeight: 1
    letterSpacing: "0.16em"
  mono:
    fontFamily: "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace"
    fontSize: "12px"
    fontWeight: 400
    lineHeight: 1.6
rounded:
  sharp: "2px"
  full: "9999px"
spacing:
  hair: "2px"
  tight: "4px"
  snug: "8px"
  base: "12px"
  card: "16px"
  section: "24px"
  hero: "32px"
components:
  button-primary:
    backgroundColor: "{colors.brand-blue}"
    textColor: "oklch(0.985 0 0)"
    rounded: "{rounded.sharp}"
    height: "36px"
    padding: "8px 16px"
  button-primary-hover:
    backgroundColor: "{colors.brand-blue-press}"
  button-accent:
    backgroundColor: "{colors.signal-amber}"
    textColor: "{colors.surface-floor}"
    rounded: "{rounded.sharp}"
    height: "36px"
    padding: "8px 16px"
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.ink}"
    rounded: "{rounded.sharp}"
    height: "36px"
    padding: "8px 16px"
  card:
    backgroundColor: "{colors.surface-raised}"
    textColor: "{colors.ink}"
    rounded: "{rounded.sharp}"
    padding: "16px"
  tile:
    backgroundColor: "{colors.surface-low}"
    textColor: "{colors.ink}"
    rounded: "{rounded.sharp}"
    padding: "12px 16px"
  list-row:
    textColor: "{colors.ink}"
    padding: "12px 16px"
  field-shell:
    backgroundColor: "{colors.surface-raised}"
    textColor: "{colors.ink}"
    rounded: "{rounded.sharp}"
    padding: "10px 16px"
  eyebrow:
    textColor: "{colors.ink-muted}"
    typography: "{typography.label}"
  nav-item:
    textColor: "{colors.ink}"
    typography: "{typography.label}"
    padding: "8px 16px"
---

# Design System: SiteCMD Desktop

## Overview

**Creative North Star: "The Kinetic Console"**

SiteCMD is read like an instrument panel, not a document. It is a dark-first
console a developer runs their site and source through, and every surface earns
its place by carrying a reading: a score, a severity, a count, a trend. The name
comes from the app's own `tokens.css`, and the way to design for it is to think
like an instrument maker. Depth is built from tonal nesting, where surfaces
separate by stepping luminosity (`--surface-floor #09090b` up through
`--surface-highest #353437`) rather than by drawing lines. Borders exist, but
only as a hairline ghost (`rgba(255,255,255,0.08)`), a whisper of separation, not
structure. Corners are nearly square (a single 2px radius everywhere), which
reads as precision hardware rather than soft consumer app.

The palette is disciplined to the point of being a rule. The brand blue and its
amber accent are used sparingly, reserved for the one primary action and the
active location. The colors that do most of the talking are semantic signal
families, kept strictly separate from the brand: score (emerald to red),
severity (red / orange / yellow / blue), and per-category hues. Color here always
means something. Type is dense and small (a 13px body, 10 to 11px uppercase
eyebrows), which frees the biggest, boldest type on any screen to be a number.
Values are near-black weight and tabular, so columns of readings line up like a
gauge cluster. Motion is restrained and functional: 120 to 180ms state
transitions, a row that types itself in during a scan, a blinking terminal
cursor. Nothing decorative moves.

This is not an editorial or marketing world, and it deliberately rejects that
vocabulary: no large rounded cards, no colored pills for metadata, no soft
drop-shadows at rest, no accent bars ornamenting a header. The feeling to protect
is a calm, high-density control room where the data is the design.

**Key Characteristics:**

- Dark-first, with a fully specified light counterpart (cool slate chrome).
- Tonal nesting for depth; hairline ghost borders; shadows only on floating layers.
- One near-square 2px corner across the whole app; true circles are the only exception.
- Rare brand accent; semantic score / severity / category color does the signalling.
- Dense small type, with large black tabular numerics as the visual hero.
- OKLCH-authored tokens, dual-theme, driven entirely through CSS variables.

## Colors

The palette splits into three jobs: a rare brand voice, a large semantic signal
system, and a tonal neutral chassis that carries everything else. All values are
CSS variables from `tokens.css`; the app never hardcodes a hex.

### Primary

- **Kinetic Blue** (`oklch(0.67 0.214 259.13)` dark, `oklch(0.546 0.245 262.88)` light): the brand voice and every primary action. It fills the default `<Button>`, paints the active nav item (over a `rgba(59,130,246,0.10)` wash), and tints interactive-card hover states. It presses to `oklch(0.47 0.214 259.13)` on hover, keyboard focus, and `:active`. The dark-theme value is lighter than the light theme's because this same blue is also used as small text on dark surfaces (nav, cards, stage labels), and needed the extra lightness to clear 4.5:1 AA contrast there.

### Secondary

- **Signal Amber** (`oklch(0.769 0.188 70.08)` dark, `oklch(0.705 0.213 47.604)` light): the accent, used even more sparingly than blue. It marks the accent button, a card's title icon, and the "attention" score band. It is a spotlight, not a second brand color.

### Tertiary - Semantic Signal

These are not decoration and never substitute for the brand accent. Each family maps a meaning to a hue and is applied as text or a 10 to 15% wash, never as a solid pill.

- **Score band** - Excellent `#34d399`, Good `#fbbf24`, Attention `#fb923c`, Poor `#f87171`, Critical `#ef4444`. Drives the ScoreRing fill and every score readout.
- **Severity** - Critical `#ef4444`, High `#f97316`, Medium `#eab308`, Low `#60a5fa`. Ranks findings in issue rows and eyebrows.
- **Category** - Security `#ef4444`, Performance `#3b82f6`, SEO `#34d399`, Accessibility `#a78bfa`, Compliance `#fbbf24`, Config `#94a3b8`, Polish `#f472b6`, Code `#22d3ee`. Labels which engine or domain a finding came from.

### Neutral - The Tonal Chassis

The dark theme is the signature. Depth is a luminosity ladder, not a shadow stack.

- **Floor** (`#09090b`): app background and nav sidebar; the deepest plane.
- **Low / Raised / Container** (`#0e0e10` / `#131315` / `#1c1b1d`): recessed wells, standard cards, and lifted rows respectively.
- **High / Highest** (`#2a2a2c` / `#353437`): the top of the stack, for the most-lifted controls.
- **Ink** (`#e5e1e4`): warm off-white body text. **Ink Muted** (`#b0b3c0`): metadata and secondary text, held at AAA contrast (7.8:1+) even at 10 to 13px.
- **Ghost Border** (`rgba(255,255,255,0.08)`): the one border value; a hairline, never a divider of record.
- **Obsidian Glass** (`rgba(20,20,22,0.85)` + `blur(12px)`): popovers, dropdowns, and modal chrome.

The light theme mirrors every token in cool-tinted slate (background `oklch(0.976 0.004 264.54)`, ink `oklch(0.13 0.028 261.69)`), keeping the same blue/amber voice.

### Named Rules

**The One Voice Rule.** The brand blue and amber appear on a small fraction of any screen - the primary action and the active location. Their rarity is what makes them read as "act here."

**The Signal, Not Accent Rule.** Score, severity, and category colors are semantic. Never repaint them for emphasis and never let them stand in for the brand accent. If a color is on screen, a reader should be able to name what it means.

**The No Hardcoded Hex Rule.** Every color resolves through a `tokens.css` variable. A literal hex in a component is a bug the guardrail fails on.

## Typography

**Display / Body Font:** Inter (with `-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto` fallback), with `cv11` and `ss01` OpenType features on.
**Mono Font:** the platform mono stack (`ui-monospace, SFMono-Regular, Menlo, Consolas`) for evidence, code refs, and paths.

**Character:** One family, worked hard across weight and size rather than across typefaces. The scale is deliberately small and tight (five text sizes, two colors, two weights), which keeps the interface dense and lets numeric displays dominate by contrast.

### Hierarchy

- **Display** (900, 40px, line-height 1, tabular): the huge score / total readout (`.metric-display`); the score hero pushes to 60px. Always a number.
- **Headline** (900, 22 to 24px, tabular): KPI and delta values (`.metric-value`, `.stat-value`), and modal titles.
- **Title** (600, 14px, +0.02em): card and section headings (`.card__title`). Quiet, never shouty.
- **Body** (400, 13px, line-height 1.6): standard paragraph text (`.text-body`); muted secondary drops to 12px in `--ink-muted`.
- **Label / Eyebrow** (700, 10px, uppercase, +0.16em tracking): severity, domain, and status metadata set inline beside titles (`.eyebrow`, `.section-label`). Section labels widen tracking to 0.20em at 11px.
- **Mono** (400, 11 to 12px): inline code, hostnames, and dossier evidence blocks.

### Named Rules

**The Numbers Are the Hero Rule.** The largest, boldest type on a screen is almost always a metric. The `.metric-*` roles use weight 900 and tabular numerals so columns of readings align like a gauge cluster. Prose never competes with them for weight.

**The Eyebrow, Never a Pill Rule.** Metadata labels are uppercase tracked text that take their color from the caller (a severity or category hue). They are never wrapped in a background pill.

**The 10px Floor Rule.** 10px is the smallest text in the system. Nothing smaller ships.

## Layout

The app is a fixed desktop chrome, not a scrolling page. A 208px nav sidebar
(collapsing to 52px) and a 48px draggable top bar frame a single scrolling content
column. Density is high and consistent.

Spacing follows the `--space-*` scale (4 / 8 / 12 / 16 / 24 / 32px), and
**padding is role-owned**: a `.card`, `.panel`, `.list-row`, or `.tile` supplies
its own internal padding (cards 16px, panels 24px, rows 12x16px), so consumers do
not add one-off spacing on top. Arbitrary pixel spacing in JSX is banned by
guardrail; component and layout classes own sibling margins and gaps.

Dense tables and split panels use semantic row and divider classes so their
contents read as one continuous instrument rather than detached cards.
Responsive behavior stacks columns below the 640px and large-layout breakpoints
through `.row-responsive-*` and `.responsive-stack-row`. Scrollbars are thin
(8px) and translucent.

## Elevation & Depth

This system conveys depth through **tonal nesting**, not shadow. A surface reads
as lifted because it is a step lighter than the one behind it (`--surface-floor`
to `--surface-highest`), reinforced by a hairline ghost border and, on tiles, a
1px inset highlight (`inset 0 0 0 1px rgba(255,255,255,0.035)`). At rest, almost
nothing casts a shadow.

Real drop shadows are reserved for elements that genuinely leave the plane, where
they read as "floating above the console."

### Shadow Vocabulary

- **Floating menu** (`box-shadow: 0 8px 32px rgba(0,0,0,0.4)`): dropdowns and popovers, paired with obsidian-glass blur.
- **Modal lift** (a deep black shadow plus a faint inset edge): scan summary, command palette, and dialog panels.
- **Deep sheet** (`box-shadow: 0 24px 80px rgba(0,0,0,0.45)`): the scheduled-scan sheet, the most-lifted layer.
- **Focus ring** (`0 0 0 2px var(--background), 0 0 0 4px var(--ring)`): a doubled outline on `:focus-visible` for keyboard users.

### Named Rules

**The Tonal Nesting Rule.** Depth is a luminosity step, not a border and not a shadow. To lift an element, move it up the surface ladder; do not draw a heavier line around it.

**The Shadows Float, Surfaces Don't Rule.** If an element sits in the layout, it has no drop shadow. Drop shadows belong only to dropdowns, popovers, and modals that overlay the plane.

## Shapes

The form language is precise and near-square. A single custom property,
`--app-radius: 2px`, gives cards, panels, buttons, inputs, and tiles one uniform,
hardware-like curvature. The only exceptions are genuine circles - score
badges, avatars, toggle thumbs, round icon badges, and status dots - which use a
full `9999px` radius.
There is no middle ground between 2px and fully round, and that binary is the
point.

Separation, where a border alone is too weak, comes from the ghost border plus a
tonal shift, or from semantic hairline dividers between rows, not from heavier
strokes.

### Named Rules

**The One Corner Rule.** 2px everywhere, or fully round for true circles. Never introduce an intermediate radius; the uniform corner is a signature.

## Components

For each component, lead with its character, then shape, color, and states.

### Buttons

Rendered only through the shared `<Button>` component, which emits compact `.btn btn--<variant> btn--<size>` classes. Confident but not loud.

- **Shape:** 2px corner (`var(--radius)`), 36px tall default (`.btn`), 8x16px padding, weight 500 with 150ms color/transform transitions.
- **Primary (`btn--default`):** brand-button blue, weight bumped to 700 and +0.01em tracking; hovers and presses to the deeper blue. The one high-emphasis action per view.
- **Accent (`btn--accent`):** signal amber; brightens 10% and lifts a small shadow on hover, presses to `scale(0.97)`.
- **Secondary / Outline / Ghost:** tonal fills (`--secondary`, a bordered `--card`, or transparent) that resolve to the `--accent` surface on hover. These carry a faint `0 1px 2px rgba(0,0,0,0.05)` seat shadow except ghost.
- **Semantic (`btn--destructive` / `success` / `warning`):** transparent with a color-mixed border and text in the matching signal hue; the wash deepens on hover.
- **Sizes:** sm 32px / default 36px / lg 40px / icon 36px square.
- **Focus:** the doubled focus ring (2px background gap, then a 4px `--ring`).

### Cards / Containers

The tonal chassis. Flat at rest, lifted by surface tone and a ghost border.

- **Corner:** 2px through `var(--radius)`.
- **Background:** `--card` (`#131315`) for `.card`, `--surface-low` for `.tile`, `--muted` for the muted variant.
- **Border:** ghost border (`rgba(255,255,255,0.08)`); tiles add a 1px inset highlight instead.
- **Padding:** role-owned - card 16px (compact 12, spacious 20), panel 24px.
- **Interactive:** `.card--interactive` hovers to a 10% primary-color wash and presses to `scale(0.99)`. Static display cards get no hover.

### Inputs / Fields

Recessed wells that read as "type here."

- **Style:** `.field-shell` is a `--card` row with a ghost border and 10x16px padding; `.field-control` sits on `--background` with a hairline border, both at 2px corners and 13px text.
- **Focus:** a 2px `--ring` (`focus-within` for shells, with a 1px offset), no color change to the fill.
- **Wells:** a control nested in a card drops to `--background` (`.control-well`) so elevation, not a heavier border, signals that it is editable.
- **Disabled:** 50% opacity, `not-allowed` cursor.

### Navigation

The sidebar is the app's spine, styled as a quiet console index.

- **Style:** `.nav-item` is uppercase 12px, `font-bold`, wide letter-spacing, 8x16px, on the `--surface` floor with an 18px brand-tinted icon.
- **States:** active is brand-blue text over a `rgba(59,130,246,0.10)` wash; inactive is dimmed ink that warms to brand-blue over a lighter blue wash on hover. Count badges sit right-aligned, tabular, and take a severity hue when they carry a warning.
- **Collapsed:** at 52px the labels drop and icons center.

### Eyebrow (signature label)

The metadata primitive that keeps the "no pills" rule enforceable: 10px uppercase, 0.16em tracking, `font-bold`, color supplied by the caller (a severity or category hue). Plain text, inline, never boxed.

### ScoreRing (signature component)

The instrument made literal. A circular SVG gauge with a `--muted` track and a fill in the score-band color, a large weight-900 tabular value centered inside, and a small `/100` denominator beneath. The same component powers every score hero so the reading is identical wherever it appears. Default 96px, stroke scales with size.

## Do's and Don'ts

### Do:

- **Do** build depth by stepping the surface ladder (`--surface-floor` to `--surface-highest`) plus a ghost border, not by adding shadows or heavier lines.
- **Do** keep every corner at 2px, and reserve the full `9999px` radius for true circles only.
- **Do** render every clickable action through the `<Button>` component, and compose UI from the named primitives (`card`, `panel`, `tile`, `list-row`, `eyebrow`, `field-shell`) before writing new classes.
- **Do** let metric numbers be the largest type on screen and use the `.metric-*` roles.
- **Do** take semantic color from the score / severity / category tokens, applied as text or a 10 to 15% wash.
- **Do** name classes by product role (`integration-card`), not visual recipe (`card-with-shadow`).

### Don't:

- **Don't** wrap severity, scope, or status metadata in a rounded pill with a background color. Use `.eyebrow` plus a color class.
- **Don't** hardcode a hex or use `text-zinc-*`; resolve through `tokens.css` variables.
- **Don't** use inline `style=` attributes. The escape hatches are `progress-bar`'s runtime width, `score-ring`'s size-derived geometry, and the react-pdf `ReportPDFSections`.
- **Don't** put hover effects on non-clickable elements. A hover state means "this responds to a click."
- **Don't** add a colored accent bar at the top of a card, hero, or panel.
- **Don't** spend the brand blue or amber broadly; keep them on the primary action and the active location.
- **Don't** introduce an intermediate corner radius, a second display typeface, or text below 10px.
