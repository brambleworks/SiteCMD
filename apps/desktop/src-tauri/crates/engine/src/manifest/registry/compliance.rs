//! Compliance capability registry.
//! Legal-document checks remain page-scoped because their verdict includes the page's links.

use crate::manifest::Entry;

pub const ENTRIES: &[Entry] = &[
    Entry::new("compliance.accessibility_statement"),
    Entry::new("compliance.ccpa_notice"),
    Entry::new("compliance.consent_mode"),
    Entry::new("compliance.cookie_consent"),
    Entry::new("compliance.cookie_expiration"),
    Entry::new("compliance.data_controller_contact"),
    Entry::new("compliance.dnt_respect"),
    Entry::new("compliance.form_consent"),
    // Revision 2 distinguishes unreachable probes from observed absence.
    Entry::new("compliance.privacy_policy").probe().revision(2),
    Entry::new("compliance.terms").probe().revision(2),
    Entry::new("compliance.trackers"),
];
