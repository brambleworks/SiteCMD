import type { FixGuideEntry } from "./types";

export const COMPLIANCE_FIX_GUIDES: Record<string, FixGuideEntry> = {
  "compliance.cookie_consent": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Inventory which cookies, storage, and vendor requests actually fire before and after interaction, and confirm with your privacy owner which jurisdictions and purposes require prior consent; the answer is not universal. Where consent is required, gate scripts at the real execution boundary, not with a banner alone, and verify each choice by inspecting Network and Storage on fresh visits.",
    ],
  },
  "compliance.privacy_policy": {
    effort: "involved",
    effortMinutes: 30,
    default: [
      "Confirm which privacy laws apply and inventory the data the site actually collects across forms, accounts, logs, analytics, payments, and vendors. Publish a notice that matches that reality (identity and contact, purposes, recipients, retention, and user rights to the extent the governing rules require), avoid templates naming tools you do not use, link it from persistent navigation, and have the privacy or legal owner review it.",
    ],
  },
  "compliance.terms": {
    effort: "involved",
    effortMinutes: 30,
    default: [
      "First determine whether the site's accounts, payments, user content, or other relationship needs contractual terms in the applicable jurisdictions; a brochure site may not. If terms are needed, draft them around the service that actually exists, in plain language, and choose the assent flow with legal review, since enforceable contract formation can require a clear affirmative action while a footer link may be enough for some notices.",
    ],
  },
  "compliance.accessibility_statement": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Confirm whether a public-sector rule, procurement contract, covered-service law, or internal commitment requires an accessibility statement; requirements vary by organization and jurisdiction. If one is required, base it on an actual assessment (scope, standard and version, testing method, date), do not claim WCAG conformance from an automated scan alone, list known limitations honestly, and publish it at a stable location with a working contact path for barrier reports.",
    ],
  },
  "compliance.ccpa_notice": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Confirm whether the business is subject to CCPA/CPRA and whether it sells or shares personal information, including ad-tech sharing. If the law applies, add a clear 'Do Not Sell or Share My Personal Information' opt-out path in the footer and privacy policy, support Global Privacy Control if it applies to your opt-out workflow, and describe the collected categories, purposes, and recipients in the privacy policy.",
    ],
  },
  "compliance.cookie_expiration": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Inspect each flagged cookie in DevTools and identify its owner, purpose, and actual Max-Age; SiteCMD's one-year flag is a review heuristic, not a universal legal cutoff. Set the shortest lifetime the purpose justifies (session, remember-me, fraud, and analytics cookies can need different periods) in the server, framework, or vendor setting that owns the cookie, then verify the deployed Set-Cookie header.",
    ],
  },
  "compliance.data_controller_contact": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Confirm which notice regime applies and which entity is the responsible controller or business, then publish the identity and contact details that regime requires in the privacy notice, with a monitored channel for privacy and rights requests. Name a representative or Data Protection Officer only when the role exists or is legally required; do not invent one to clear the finding.",
    ],
  },
  "compliance.dnt_respect": {
    effort: "moderate",
    effortMinutes: 10,
    default: [
      "Separate the signals: DNT is generally a voluntary browser preference, while GPC can be a legally recognized opt-out for covered businesses in some jurisdictions. Determine which activities each recognized signal must change, process it at server, tag-manager, and SDK boundaries rather than checking one browser property, and describe the actual behavior in the privacy notice without promising more than the implementation does. Test requests with and without Sec-GPC.",
    ],
  },
  "compliance.form_consent": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Confirm what the form does and identify the applicable jurisdictions, collection purpose, and lawful basis with your privacy owner or counsel before changing it. Consent is not automatically required for every use of an email address, and asking for consent can be misleading when the service would process the data on another basis anyway.",
      "Provide the collection information the applicable law requires at the form or through a clear just-in-time link. If consent is the applicable basis, make the choice specific, informed, optional, and affirmative rather than pre-checked or bundled, and honor withdrawal; obtain jurisdiction-specific review before treating the scanner result as a compliance conclusion.",
    ],
  },
  "compliance.trackers": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Treat the finding as an inventory lead: confirm in DevTools Network and Storage which provider code actually executes and what it sends, since a provider string alone does not establish that personal data is collected. Remove vendors that are not needed; for the rest, implement the disclosure, consent, opt-out, or regional gating the actual use case and governing rules require, and verify every preference state on the deployed site.",
    ],
  },
  "compliance.consent_mode": {
    effort: "moderate",
    effortMinutes: 20,
    default: [
      "Treat this as a Google-product configuration review: identify which Google tags receive data and which visitors require a choice under your policy. If Consent Mode applies, send the v2 consent types with regional defaults set before measurement commands (Google's example values are not a universal instruction), update consent state when a visitor changes the choice, and verify the actual behavior with Tag Assistant plus Network and Storage inspection.",
    ],
  },
};
