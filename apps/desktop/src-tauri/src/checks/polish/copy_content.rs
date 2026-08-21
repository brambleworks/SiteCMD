//! Heuristic copy and content signals for Polish Scan.
//!
//! Headings and hero copy receive full buzzword weight; body copy receives half.

use super::{PolishContext, PolishResult, SignalCategory, SignalWeight};
use regex::Regex;
use std::sync::LazyLock;

const CATEGORY: SignalCategory = SignalCategory::CopyContent;

/// Matches em dash characters (Unicode, HTML entity, and raw)
static EM_DASH_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\u{2014}|&mdash;|&#8212;)").expect("em dash regex"));

/// Approximate sentence boundary: period/question/exclamation followed by space or end
static SENTENCE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[.!?]\s").expect("sentence regex"));

/// Extracts text content from heading tags (h1-h6)
static HEADING_TEXT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<h[1-6][^>]*>(.*?)</h[1-6]>").expect("heading text regex"));

/// Strips HTML tags to get plain text
static HTML_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<[^>]+>").expect("html tag regex"));

/// A `<script>...</script>` block, removed whole so JS is never read as prose.
static SCRIPT_BLOCK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<script\b[^>]*>.*?</script>").expect("script block regex"));

/// A `<style>...</style>` block, removed whole so CSS declarations (e.g.
/// `transform:`) are never counted as marketing buzzwords.
static STYLE_BLOCK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<style\b[^>]*>.*?</style>").expect("style block regex"));

/// Matches "whether you're a" or "whether you are a" patterns
static INCLUSIVE_FRAMING_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)whether you(?:'re| are) a(?-u:\b)").expect("inclusive framing regex")
});

/// Rendered emoji bases, excluding selectors and skin-tone modifiers.
static EMOJI_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[\x{1F300}-\x{1F3FA}\x{1F400}-\x{1F9FF}\x{2600}-\x{26FF}\x{2700}-\x{27BF}\x{1FA00}-\x{1FA6F}\x{1FA70}-\x{1FAFF}]")
        .expect("emoji regex")
});

/// Heading and list text used for feature-section emoji detection.
static FEATURE_H2_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<h2(?-u:\b)[^>]*>(.*?)</h2>").expect("feature h2 regex"));
static FEATURE_H3_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<h3(?-u:\b)[^>]*>(.*?)</h3>").expect("feature h3 regex"));
static FEATURE_LI_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<li(?-u:\b)[^>]*>(.*?)</li>").expect("feature li regex"));

/// Known AI-generated header patterns
static AI_HEADER_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // "The [Adjective] Way to [Verb]"
        Regex::new(r"(?i)^the\s+[A-Za-z0-9_]+\s+way\s+to\s+[A-Za-z0-9_]+").unwrap(),
        // "From [X] to [Y]"
        Regex::new(r"(?i)^from\s+[A-Za-z0-9_]+\s+to\s+[A-Za-z0-9_]+").unwrap(),
        // "[Marketing verb] Your [Noun]". A bare any-word form matched
        // functional UI copy - "Reset your password", "Change your
        // email"  - so only hype verbs count.
        Regex::new(r"(?i)^(?:supercharge|elevate|empower|unleash|unlock|boost|accelerate|streamline|simplify|master|automate|maximize|grow|scale)\s+your\s+[A-Za-z0-9_]+$").unwrap(),
        // Two-word imperative fragments: "Ship Faster." "Build Better."
        Regex::new(r"(?i)^[A-Za-z0-9_]+\s+[A-Za-z0-9_]+\.\s*$").unwrap(),
        // "Reimagine/Revolutionize/Transform Your [Noun]"
        Regex::new(r"(?i)^(reimagine|revolutionize|transform|redefine|reinvent)\s+your\s+")
            .unwrap(),
        // "Why [Product/Feature]?"
        Regex::new(r"(?i)^why\s+[A-Za-z0-9_]+.*\?$").unwrap(),
        // "[Noun]: Reimagined/Redefined/Reinvented"
        Regex::new(r"(?i):\s*(reimagined|redefined|reinvented)\s*$").unwrap(),
        // "The Future of [Noun]"
        Regex::new(r"(?i)^the\s+future\s+of\s+").unwrap(),
    ]
});

/// Tier 1 (strong signals, weight 3)
const TIER_1_WORDS: &[&str] = &[
    "seamlessly",
    "effortlessly",
    "elevate",
    "harness",
    "delve",
    "tapestry",
    "reimagine",
    "revolutionize",
    "cutting-edge",
    "empower",
    "supercharge",
    "game-changer",
    "next-generation",
];

/// Tier 2 (moderate signals, weight 2)
const TIER_2_WORDS: &[&str] = &[
    "streamline",
    "leverage",
    "robust",
    "unlock",
    "transform",
    "innovative",
    "state-of-the-art",
    "unparalleled",
    "comprehensive",
    "dynamic",
    "intuitive",
    "scalable",
    "synergy",
];

/// Tier 3 (weak signals, weight 1 - only meaningful in clusters)
const TIER_3_WORDS: &[&str] = &[
    "enhance",
    "optimize",
    "discover",
    "explore",
    "journey",
    "solution",
    "powerful",
    "intelligent",
    "smart",
    "modern",
    "sleek",
    "beautiful",
    "stunning",
];

/// Extract prose from HTML without script or style content.
fn strip_html(html: &str) -> String {
    let without_script = SCRIPT_BLOCK_RE.replace_all(html, " ");
    let without_code = STYLE_BLOCK_RE.replace_all(&without_script, " ");
    let text = HTML_TAG_RE.replace_all(&without_code, " ");
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

/// Extract heading text content from HTML.
fn extract_heading_text(html: &str) -> Vec<String> {
    HEADING_TEXT_RE
        .captures_iter(html)
        .map(|cap| strip_html(&cap[1]).trim().to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

/// Count case-insensitive, whole-word occurrences.
fn count_word(text: &str, word: &str) -> usize {
    use std::collections::HashMap;
    use std::sync::Mutex;
    static CACHE: std::sync::LazyLock<Mutex<HashMap<String, regex::Regex>>> =
        std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));
    let word_lower = word.to_lowercase();
    let pattern = format!(r"(?i)\b{}\b", regex::escape(&word_lower));
    let mut cache = CACHE.lock().expect("count_word regex cache poisoned");
    let re = cache
        .entry(pattern.clone())
        .or_insert_with(|| regex::Regex::new(&pattern).expect("compiled \\b<word>\\b regex"));
    re.find_iter(text).count()
}

/// Detect unusually dense em-dash use.
/// Threshold: more than one per three sentences. Weight: High.
pub fn em_dash_density(ctx: &PolishContext) -> PolishResult {
    let text = strip_html(&ctx.html);
    // Count only prose because the denominator is prose sentences.
    let em_dash_count = EM_DASH_RE.find_iter(&text).count();

    let sentences = SENTENCE_RE.find_iter(&text).count().max(1);

    if em_dash_count == 0 {
        return PolishResult::clear(
            "em-dash-density",
            "High Em-Dash Density",
            SignalWeight::High,
            CATEGORY,
        );
    }

    let ratio = em_dash_count as f64 / sentences as f64;

    // >1 per 3 sentences = >0.333
    if ratio > 0.333 {
        PolishResult::fired(
            "em-dash-density",
            "High Em-Dash Density",
            SignalWeight::High,
            CATEGORY,
            format!(
                "1 em dash per {:.1} sentences ({} em dashes, ~{} sentences)",
                1.0 / ratio,
                em_dash_count,
                sentences
            ),
            serde_json::json!({
                "em_dash_count": em_dash_count,
                "sentence_count": sentences,
                "ratio": (ratio * 1000.0).round() / 1000.0,
            }),
        )
    } else {
        PolishResult::clear(
            "em-dash-density",
            "High Em-Dash Density",
            SignalWeight::High,
            CATEGORY,
        )
    }
}

/// Flag weighted AI-favored phrasing above 15 points (High, 15).
pub fn ai_buzzword_dictionary(ctx: &PolishContext) -> PolishResult {
    let heading_text = extract_heading_text(&ctx.html).join(" ");
    let full_text = strip_html(&ctx.html);

    // Remove heading text from full text to get body text
    let body_text = full_text.clone(); // approximate - headers included but body weight applied

    let mut total_score = 0.0f64;
    let mut found_words: Vec<(String, usize, u32)> = Vec::new(); // (word, count, tier_weight)

    let check_word = |word: &str, tier_weight: u32| -> (usize, f64) {
        let heading_count = count_word(&heading_text, word);
        let body_count = count_word(&body_text, word);
        // Headings receive full weight without being counted twice in the total.
        let total_count = body_count;
        let score = (heading_count as f64 * tier_weight as f64)
            + ((body_count.saturating_sub(heading_count)) as f64 * tier_weight as f64 * 0.5);
        (total_count, score)
    };

    for word in TIER_1_WORDS {
        let (count, score) = check_word(word, 3);
        if count > 0 {
            total_score += score;
            found_words.push((word.to_string(), count, 3));
        }
    }
    for word in TIER_2_WORDS {
        let (count, score) = check_word(word, 2);
        if count > 0 {
            total_score += score;
            found_words.push((word.to_string(), count, 2));
        }
    }
    for word in TIER_3_WORDS {
        let (count, score) = check_word(word, 1);
        if count > 0 {
            total_score += score;
            found_words.push((word.to_string(), count, 1));
        }
    }

    // Sort by tier weight desc, then count desc
    found_words.sort_by(|a, b| b.2.cmp(&a.2).then(b.1.cmp(&a.1)));

    if total_score > 15.0 {
        let top_words: Vec<String> = found_words
            .iter()
            .take(5)
            .map(|(w, c, _)| format!("\"{}\" x{}", w, c))
            .collect();
        PolishResult::fired(
            "ai-buzzword-dictionary",
            "High Marketing Buzzword Density",
            SignalWeight::High,
            CATEGORY,
            format!(
                "Buzzword score {:.0} ({})",
                total_score,
                top_words.join(", ")
            ),
            serde_json::json!({
                "total_score": total_score.round() as u32,
                "words_found": found_words.len(),
                "top_words": found_words.iter().take(10)
                    .map(|(w, c, t)| serde_json::json!({"word": w, "count": c, "tier": t}))
                    .collect::<Vec<_>>(),
            }),
        )
    } else {
        PolishResult::clear(
            "ai-buzzword-dictionary",
            "High Marketing Buzzword Density",
            SignalWeight::High,
            CATEGORY,
        )
    }
}

/// Flag two or more formulaic headings (Medium, 8).
pub fn ai_header_formulas(ctx: &PolishContext) -> PolishResult {
    let headings = extract_heading_text(&ctx.html);
    let mut matched: Vec<String> = Vec::new();

    for heading in &headings {
        let trimmed = heading.trim();
        for pattern in AI_HEADER_PATTERNS.iter() {
            if pattern.is_match(trimmed) {
                matched.push(trimmed.to_string());
                break; // only count each heading once
            }
        }
    }

    if matched.len() >= 2 {
        PolishResult::fired(
            "ai-header-formulas",
            "Common Marketing Headline Patterns",
            SignalWeight::Medium,
            CATEGORY,
            format!("{} headers match AI patterns", matched.len()),
            serde_json::json!({
                "matched_count": matched.len(),
                "matched_headers": matched.iter().take(5).collect::<Vec<_>>(),
            }),
        )
    } else {
        PolishResult::clear(
            "ai-header-formulas",
            "Common Marketing Headline Patterns",
            SignalWeight::Medium,
            CATEGORY,
        )
    }
}

/// Flag generic "whether you're X or Y" framing (Medium, 8).
pub fn inclusive_framing(ctx: &PolishContext) -> PolishResult {
    let text = strip_html(&ctx.html);
    let matches = INCLUSIVE_FRAMING_RE.find_iter(&text).count();

    if matches > 0 {
        PolishResult::fired(
            "inclusive-framing",
            "Generic Audience Phrasing Detected",
            SignalWeight::Medium,
            CATEGORY,
            format!(
                "{} occurrence{} of \"whether you're a\" pattern",
                matches,
                if matches != 1 { "s" } else { "" }
            ),
            serde_json::json!({ "occurrences": matches }),
        )
    } else {
        PolishResult::clear(
            "inclusive-framing",
            "Generic Audience Phrasing Detected",
            SignalWeight::Medium,
            CATEGORY,
        )
    }
}

/// Flag three or more emoji in feature headings and lists (Medium, 8).
pub fn emoji_as_icons(ctx: &PolishContext) -> PolishResult {
    let mut emoji_count = 0usize;

    for re in [&*FEATURE_H2_RE, &*FEATURE_H3_RE, &*FEATURE_LI_RE] {
        for cap in re.captures_iter(&ctx.html) {
            let content = &cap[1];
            emoji_count += EMOJI_RE.find_iter(content).count();
        }
    }

    if emoji_count >= 3 {
        PolishResult::fired(
            "emoji-as-icons",
            "Emoji as Icons",
            SignalWeight::Medium,
            CATEGORY,
            format!("{} emoji in headings or list items", emoji_count),
            serde_json::json!({ "emoji_count": emoji_count }),
        )
    } else {
        PolishResult::clear(
            "emoji-as-icons",
            "Emoji as Icons",
            SignalWeight::Medium,
            CATEGORY,
        )
    }
}
/// Detect a three-column feature grid only when emoji headings add a second signal.
pub fn three_column_grid(ctx: &PolishContext) -> PolishResult {
    let html_lower = ctx.html_lower();
    let has_grid_container = html_lower.contains("grid-cols-3")
        || html_lower.contains("grid cols-3")
        || Regex::new(r#"(?i)class\s*=\s*["'][^"']*\bgrid\b[^"']*["']"#)
            .ok()
            .map(|r| r.is_match(&ctx.html))
            .unwrap_or(false);

    // Count h2/h3 tags that are immediately followed (within 200 chars) by a <p>
    let heading_p_pattern =
        Regex::new(r"(?is)<h[23][^>]*>.*?</h[23]>\s*(?:.{0,200}?)<p(?-u:\b)").ok();
    let heading_p_count = heading_p_pattern
        .map(|r| r.find_iter(&ctx.html).count())
        .unwrap_or(0);

    // Check for emoji before headings (emoji + heading pattern repeated 3 times)
    let emoji_heading_count = Regex::new(r"(?s)[\x{1F300}-\x{1F9FF}\x{2600}-\x{26FF}].*?<h[23]")
        .ok()
        .map(|r| r.find_iter(&ctx.html).count())
        .unwrap_or(0);

    // Require grid, repeated cards, and emoji headings together; grid alone is
    // too common to be meaningful.
    let detected = has_grid_container && heading_p_count >= 3 && emoji_heading_count >= 3;

    if detected {
        // A generic grid does not prove a three-column layout.
        let has_three_col_class =
            html_lower.contains("grid-cols-3") || html_lower.contains("grid cols-3");
        let detail = if has_three_col_class {
            "3-column feature grid with emoji headings and heading + paragraph cards detected"
        } else {
            "Feature-grid layout with emoji headings and heading + paragraph cards detected"
        };
        PolishResult::fired(
            "three-column-grid",
            "Three-Column Feature Pattern",
            SignalWeight::LowMedium,
            CATEGORY,
            detail.to_string(),
            serde_json::json!({
                "has_grid_container": has_grid_container,
                "has_three_column_class": has_three_col_class,
                "heading_paragraph_pairs": heading_p_count,
                "emoji_heading_pairs": emoji_heading_count,
            }),
        )
    } else {
        PolishResult::clear(
            "three-column-grid",
            "Three-Column Feature Pattern",
            SignalWeight::LowMedium,
            CATEGORY,
        )
    }
}

#[cfg(test)]
mod tests;
