//! SEO manifest entries, including explicit unsupported hosted lanes.

use crate::manifest::Entry;

pub const ENTRIES: &[Entry] = &[
    Entry::new("seo.ai_crawler_blocking").probe().origin(),
    Entry::new("seo.broken_external_links").probe(),
    Entry::new("seo.broken_links").probe(),
    Entry::new("seo.canonical_mismatch"),
    Entry::new("seo.citation_meta"),
    Entry::new("seo.content_freshness"),
    Entry::new("seo.faq_schema"),
    Entry::new("seo.headings.h1"),
    Entry::new("seo.headings.hierarchy"),
    Entry::new("seo.js_only_content"),
    Entry::new("seo.llms_txt").probe().origin(),
    Entry::new("seo.meta_conflicts"),
    Entry::new("seo.og_image_status").probe(),
    Entry::new("seo.organization_identity"),
    Entry::new("seo.page_speed_hints"),
    Entry::new("seo.robots_txt").probe().origin(),
    Entry::new("seo.semantic_html"),
    Entry::new("seo.sitemap").probe().origin(),
    Entry::new("seo.sitemap_freshness").probe().origin(),
    Entry::new("seo.source_citations"),
    // Structured-data verdicts depend only on the page artifact and versioned
    // in-repo profiles, so they remain page-scoped rather than external-corpus rows.
    Entry::new("seo.structured_data"),
    Entry::new("seo.structured_data.incomplete"),
    Entry::new("seo.structured_data.invalid"),
    Entry::new("seo.temporary_redirect").probe(),
    Entry::new("seo.thin_content"),
    Entry::new("seo.url_structure"),
    // Cross-page verdicts are session-scoped so partial runs cannot claim facts
    // about routes they did not scan.
    Entry::new("seo.canonical_loop").session().unsupported(),
    Entry::new("seo.duplicate_description_across_pages")
        .session()
        .unsupported(),
    Entry::new("seo.duplicate_h1").session().unsupported(),
    Entry::new("seo.duplicate_title_across_pages")
        .session()
        .unsupported(),
    Entry::new("seo.hreflang_reciprocity")
        .session()
        .unsupported(),
    Entry::new("seo.noindex_in_sitemap").session().unsupported(),
    Entry::new("seo.orphan_pages").session().unsupported(),
    // Head-metadata checks awaiting the engine extraction. They read the
    // served document and nothing else, so each becomes an `Artifact` row
    // with no comparison dimensions the moment its module moves.
    Entry::new("seo.canonical").unsupported(),
    Entry::new("seo.charset").unsupported(),
    Entry::new("seo.duplicate_description").unsupported(),
    Entry::new("seo.duplicate_meta").unsupported(),
    Entry::new("seo.duplicate_title").unsupported(),
    Entry::new("seo.hreflang").unsupported(),
    Entry::new("seo.meta_description").unsupported(),
    Entry::new("seo.noindex").unsupported(),
    Entry::new("seo.og_image_relative").unsupported(),
    Entry::new("seo.open_graph").unsupported(),
    Entry::new("seo.title").unsupported(),
    Entry::new("seo.twitter_cards").unsupported(),
    Entry::new("seo.viewport").unsupported(),
];
