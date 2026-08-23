import type { FixGuideEntry } from "./types";

export const ACCESSIBILITY_FIX_GUIDES: Record<string, FixGuideEntry> = {
  "accessibility.axe": {
    effort: "moderate",
    effortMinutes: 20,
    lead: "An automated Accessibility check found an element on this page that fails a known rule for assistive technology.",
    default: [
      "Inspect the flagged element in the rendered DOM using the evidence selector, then apply the remedy from the rule-specific Deque help URL to every affected node, not only the first. Do not silence axe or add unrelated ARIA to clear the finding. Re-run axe on the same rendered state and confirm keyboard and screen-reader behavior, because automated checks cannot prove complete accessibility.",
    ],
  },
  "accessibility.image_alt": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "An image on this page has no alternative text, so a screen reader has nothing meaningful to announce for it.",
    default: [
      'Give each informative image concise alternative text conveying the information or purpose not already in nearby text, and use an empty alt attribute (`alt=""`) for decorative or redundant images. Make an image-only link name its destination or action, and for a complex chart keep the alt short with the full explanation in adjacent text or a table. Verify in the accessibility tree with a screen reader.',
    ],
  },
  "accessibility.form_labels": {
    effort: "quick",
    effortMinutes: 5,
    lead: "A form field on this page has no accessible label, so a screen reader user cannot tell what it is asking for.",
    default: [
      "Give every user-facing form control an accurate accessible name, normally a persistent visible `<label>` connected by `for`/`id` or by wrapping the control. Use `aria-labelledby` or `aria-label` only when a visible label relationship cannot express the design, and never rely on placeholder text as the only name, since it disappears during entry. Test the rendered form with keyboard and a screen reader.",
    ],
  },
  "accessibility.focus_indicators": {
    effort: "quick",
    effortMinutes: 5,
    lead: "An element on this page hides the visible outline that shows where keyboard focus is, with nothing put in its place.",
    default: [
      "Inspect each surfaced outline reset in the rendered component; the static check sees source markers, not whether another rule or the browser default already supplies a visible indicator. Do not suppress the browser outline unless the same state gets an equally clear replacement, for example a `:focus-visible` outline with offset. Test Tab, Shift+Tab, and programmatic focus to confirm focus never becomes visually lost.",
    ],
  },
  "accessibility.landmarks": {
    effort: "quick",
    effortMinutes: 5,
    lead: "This page's main sections are not marked up as landmarks, so assistive technology users cannot jump directly between them.",
    default: [
      "Map the rendered page regions, then use native `<header>`, `<nav>`, `<main>`, `<aside>`, and `<footer>` where their semantics match, with one visible main landmark for the current content. Label repeated landmarks of the same type, such as primary and footer navigation, with concise unique names, and confirm the result in a screen reader's landmark list.",
    ],
  },
  "accessibility.lang": {
    effort: "quick",
    effortMinutes: 1,
    lead: "This page does not declare its language, so a screen reader may read it aloud using the wrong pronunciation rules.",
    default: [
      'Add a `lang` attribute to your `<html>` tag, for example `<html lang="en">`, using the shortest valid BCP 47 tag that accurately identifies the primary language of the page. Mark passages that switch language with their own `lang` attribute and confirm pronunciation changes at the right boundaries with a screen reader.',
    ],
  },
  "accessibility.skip_nav": {
    effort: "quick",
    effortMinutes: 5,
    lead: "A keyboard user has no way to skip repeated navigation and jump straight to this page's main content.",
    default: [
      'Inspect the rendered page for an existing skip link, landmark strategy, or other bypass mechanism; if one is already effective for supported users, validate it rather than adding a duplicate control. Otherwise add a skip link near the start of the focus order targeting the real main-content container, such as `<a href="#main-content">`, keep it available to assistive technology and clearly revealed on focus, and test that activation bypasses the repeated block from a fresh load and after route changes.',
    ],
  },
  "accessibility.link_text": {
    effort: "quick",
    effortMinutes: 5,
    lead: "A link on this page reads as something like click here, giving no clue where it actually leads out of context.",
    default: [
      "Inspect each surfaced anchor's computed accessible name in context; the static text match is a review signal, since nearby programmatic context can distinguish a short label. Prefer visible text that names the destination or result, such as 'Read the privacy policy' instead of 'Click here', and give an icon-only link a concise accessible name through visually hidden text, `aria-label`, or `aria-labelledby`. Confirm by navigating by links in a screen reader.",
    ],
  },
  "accessibility.headings": {
    effort: "quick",
    effortMinutes: 5,
    lead: "The heading levels on this page jump unexpectedly, which can confuse anyone navigating by the page's structure.",
    default: [
      "Compare the rendered headings in DOM order with the actual content structure; a numeric level skip is not automatically a WCAG failure, but it often means a section has no label or visual styling drove the tag choice. Nest subsection headings under their real parent, use CSS for appearance rather than choosing a level for font size, and confirm the outline by navigating by headings in a screen reader.",
    ],
  },
  "accessibility.autoplay": {
    effort: "quick",
    effortMinutes: 5,
    lead: "Media on this page may start playing automatically with sound, and it can be hard for a visitor to pause or stop it.",
    default: [
      "Run each surfaced media element and record whether playback actually starts, has sound, and how long it lasts; static autoplay markup alone cannot answer that. Prefer user-initiated playback, or start required video muted with controls. If audio actually plays for more than three seconds, WCAG 2.2 SC 1.4.2 requires a way to pause/stop it or control its volume; give moving content that lasts more than five seconds beside other content a pause, stop, or hide mechanism unless the motion is essential.",
    ],
  },
  "accessibility.aria_usage": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "An Accessibility attribute on this page contradicts itself or points to something missing, which can confuse assistive technology.",
    default: [
      'Start from the exact surfaced pattern; this static detector reports narrow recognized conflicts such as a focusable element marked `aria-hidden="true"` or an empty `aria-label`, not a complete ARIA audit. Either expose the operable element or make the hidden content genuinely inert, remove or correct an empty label, validate every referenced ID in the rendered DOM, and confirm role, name, state, and focus agree in the accessibility tree with a screen reader.',
    ],
  },
  "accessibility.color_contrast_hints": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "Text on this page may not stand out enough from its background for someone with low vision to read comfortably.",
    default: [
      "Measure the rendered foreground against the actual composited background for each surfaced text state, including opacity, hover/focus, and dark mode; a source color pair alone is insufficient. For WCAG 2.2 AA, ordinary text needs at least 4.5:1 and large-scale text at least 3:1, and exemptions such as logos and text in inactive controls are treated differently, so review the applicable criterion before changing tokens, then re-measure the computed states.",
    ],
  },
  "accessibility.tabindex": {
    effort: "quick",
    effortMinutes: 5,
    lead: "An element on this page forces its own place in the keyboard tab order, which can strand a keyboard user partway through.",
    default: [
      'Confirm the intended keyboard sequence before removing a positive tabindex; a value above 0 overrides the normal sequential focus order and is usually fragile. Prefer natural DOM order and native interactive elements, correcting the underlying layout when visual and DOM order disagree, and use `tabindex="0"`/`tabindex="-1"` (or roving patterns for composite widgets) instead of positive values. Test Tab, Shift+Tab, and focus restoration.',
    ],
  },
  "accessibility.viewport_zoom": {
    effort: "quick",
    effortMinutes: 2,
    lead: "This page's viewport settings block visitors from pinch-zooming in, which some people rely on to read small text.",
    default: [
      'Change the viewport meta tag to `<meta name="viewport" content="width=device-width, initial-scale=1">`, deleting `user-scalable=no` and any `maximum-scale` cap below 200%; preferably omit the cap entirely. Verify pinch zoom to at least 200% on representative phones, since WCAG 2.2 SC 1.4.4 requires text to resize to at least 200% without loss of content or functionality.',
    ],
  },
  "accessibility.empty_headings": {
    effort: "quick",
    effortMinutes: 5,
    lead: "A heading on this page has no readable text in it, so a screen reader announces nothing useful at that spot.",
    default: [
      "Inspect each surfaced heading in the rendered DOM and accessibility tree, checking hidden text, image alternatives, and ARIA naming before deciding its computed accessible name is truly empty. If it labels a real section, give it concise visible heading text; if it exists only for spacing or typography, remove the heading semantics and use the element that matches the real role of the content. Confirm by navigating by headings in a screen reader.",
    ],
  },
  "accessibility.iframe_title": {
    effort: "quick",
    effortMinutes: 5,
    lead: "An embedded frame on this page has no title, so a screen reader user has no idea what it actually contains.",
    default: [
      'Inspect each surfaced iframe in the rendered page to see whether it is exposed to assistive technology and whether runtime code already supplies a name. Give each exposed frame a concise title identifying its purpose, for example `<iframe title="Product demo video">`. For a genuinely non-user-facing frame, verify it is removed from the accessibility tree and cannot receive focus; do not assume a 1x1 size or a tracking purpose makes it exempt.',
    ],
  },
  "accessibility.redundant_alt": {
    effort: "quick",
    effortMinutes: 5,
    lead: "An image's alternative text repeats filler words like image of, adding length without any information a listener needs.",
    default: [
      'Review the purpose, nearby text, and accessible name of each image before editing; remove leading words such as "image of" or "photo of" only when the medium itself is irrelevant, and keep medium information when it matters, such as distinguishing a photograph from a rendering. If the alternative is only "image" or "photo", replace it with useful context, or use `alt=""` only after confirming the image is decorative or redundant. Confirm the announcement with a screen reader.',
    ],
  },
};
