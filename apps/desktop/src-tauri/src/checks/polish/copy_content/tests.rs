use super::*;

fn ctx(html: &str) -> PolishContext {
    PolishContext {
        url: url::Url::parse("https://example.com").unwrap(),
        html: html.to_string(),
        css: String::new(),
        html_lower_cache: std::sync::OnceLock::new(),
    }
}

#[test]
fn em_dash_fires_with_high_density() {
    // 3 em dashes in 3 sentences = 1 per sentence (above threshold of 1 per 3)
    let html = "<p>First sentence \u{2014} with a dash. Second \u{2014} also dashed. Third \u{2014} same.</p>";
    let result = em_dash_density(&ctx(html));
    assert!(result.fired, "Should fire with 1 em dash per sentence");
    assert_eq!(result.weight, SignalWeight::High);
}

#[test]
fn em_dash_clear_with_sparse_usage() {
    // 1 em dash in 10 sentences
    let html = "<p>One. Two. Three. Four. Five. Six. Seven. Eight. Nine \u{2014} dashed. Ten.</p>";
    let result = em_dash_density(&ctx(html));
    assert!(
        !result.fired,
        "Should not fire with 1 em dash in 10 sentences"
    );
}

#[test]
fn em_dash_detects_html_entities() {
    let html = "<p>First &mdash; dash. Second &mdash; dash. Third.</p>";
    let result = em_dash_density(&ctx(html));
    assert!(result.fired, "Should detect &mdash; entities");
}

#[test]
fn em_dash_density_ignores_script_and_json_ld_dashes() {
    let html = "<script type=\"application/ld+json\">{\"desc\":\"a \u{2014} b \u{2014} c \u{2014} d\"}</script><p>One. Two. Three. Four. Five. Six.</p>";
    let result = em_dash_density(&ctx(html));
    assert!(
        !result.fired,
        "script/JSON-LD dashes are not prose: {}",
        result.detail
    );
}

#[test]
fn buzzwords_fire_with_heavy_ai_copy() {
    let html = r#"
            <h1>Seamlessly Elevate Your Workflow</h1>
            <p>Our cutting-edge platform empowers you to effortlessly harness the power
            of innovative solutions. Leverage our comprehensive, state-of-the-art tools
            to revolutionize your dynamic approach. Unlock unparalleled scalable synergy
            and supercharge your next-generation journey.</p>
        "#;
    let result = ai_buzzword_dictionary(&ctx(html));
    assert!(result.fired, "Should fire with heavy AI buzzword usage");
}

#[test]
fn buzzwords_clear_with_normal_copy() {
    let html = r#"
            <h1>Project Management for Teams</h1>
            <p>Track tasks, manage deadlines, and collaborate with your team.
            Our tool helps you stay organized and ship on time.</p>
        "#;
    let result = ai_buzzword_dictionary(&ctx(html));
    assert!(!result.fired, "Should not fire with normal human copy");
}

#[test]
fn buzzwords_ignore_css_in_style_blocks() {
    let css =
        "  .a{transform:translateY(0)} .b{transform:scale(1)} .c{transform:rotate(0)}\n".repeat(15);
    let html = format!(
        "<h1>Task tracker</h1><p>Track your tasks and ship on time.</p><style>{css}</style>"
    );
    let result = ai_buzzword_dictionary(&ctx(&html));
    assert!(
        !result.fired,
        "CSS transform declarations must not count as buzzwords"
    );
}

#[test]
fn buzzword_counts_do_not_double_count_headings() {
    let html = r#"
            <h1>Seamlessly Elevate Your Workflow</h1>
            <p>Our cutting-edge platform empowers you to effortlessly harness the power
            of innovative solutions. Leverage our comprehensive, state-of-the-art tools
            to revolutionize your dynamic approach. Unlock unparalleled scalable synergy
            and supercharge your next-generation journey.</p>
        "#;
    let result = ai_buzzword_dictionary(&ctx(html));
    assert!(result.fired, "{}", result.detail);
    // "elevate" appears exactly once, in the heading only.
    let elevate_count = result.data["top_words"]
        .as_array()
        .expect("top_words array")
        .iter()
        .find(|w| w["word"].as_str() == Some("elevate"))
        .and_then(|w| w["count"].as_u64())
        .expect("elevate entry");
    assert_eq!(
        elevate_count, 1,
        "one heading occurrence must count once, not twice"
    );
}

#[test]
fn header_formulas_fire_with_ai_patterns() {
    let html = r#"
            <h1>The Future of Project Management</h1>
            <h2>From Chaos to Clarity</h2>
            <h2>Why TaskFlow?</h2>
        "#;
    let result = ai_header_formulas(&ctx(html));
    assert!(result.fired, "Should fire with 3 AI-pattern headers");
}

#[test]
fn header_formulas_clear_with_normal_headers() {
    let html = r#"
            <h1>TaskFlow - Team Project Management</h1>
            <h2>Features</h2>
            <h2>Pricing</h2>
            <h2>About Us</h2>
        "#;
    let result = ai_header_formulas(&ctx(html));
    assert!(!result.fired, "Should not fire with normal headers");
}

#[test]
fn header_formulas_clear_with_functional_ui_copy() {
    let html = r#"
            <h1>Reset your password</h1>
            <h2>Change your email</h2>
            <h2>Manage your subscriptions</h2>
        "#;
    let result = ai_header_formulas(&ctx(html));
    assert!(
        !result.fired,
        "functional UI headings are not marketing formulas: {}",
        result.detail
    );
}

#[test]
fn header_formulas_still_fire_on_hype_verbs() {
    let html = r#"
            <h1>Supercharge your workflow</h1>
            <h2>Unlock your potential</h2>
        "#;
    let result = ai_header_formulas(&ctx(html));
    assert!(result.fired, "hype-verb headlines must still fire");
}

#[test]
fn inclusive_framing_fires_with_pattern() {
    let html = "<p>Whether you're a designer or a developer, our tool has you covered.</p>";
    let result = inclusive_framing(&ctx(html));
    assert!(result.fired, "Should fire with 'whether you're a' pattern");
}

#[test]
fn inclusive_framing_clear_without_pattern() {
    let html = "<p>This tool is designed for frontend developers working with React.</p>";
    let result = inclusive_framing(&ctx(html));
    assert!(!result.fired, "Should not fire without the pattern");
}

#[test]
fn emoji_icons_fires_with_emoji_in_features() {
    let html = r#"
            <h2>🚀 Fast Deployment</h2>
            <h2>🔒 Secure by Default</h2>
            <h2>📊 Real-time Analytics</h2>
        "#;
    let result = emoji_as_icons(&ctx(html));
    assert!(result.fired, "Should fire with 3+ emoji in headings");
}

#[test]
fn emoji_icons_clear_with_no_emoji() {
    let html = "<h2>Fast Deployment</h2><h2>Security</h2><h2>Analytics</h2>";
    let result = emoji_as_icons(&ctx(html));
    assert!(!result.fired, "Should not fire without emoji");
}

#[test]
fn emoji_variation_selectors_do_not_double_count() {
    let html = "<h2>\u{2699}\u{FE0F} Settings</h2><h2>\u{2764}\u{FE0F} Loved by teams</h2>";
    let result = emoji_as_icons(&ctx(html));
    assert!(
        !result.fired,
        "2 rendered emoji must count as 2, not 4: {}",
        result.detail
    );
}

#[test]
fn skin_tone_modifiers_do_not_double_count() {
    let html = "<h2>\u{1F44D}\u{1F3FD} Approved</h2><h2>\u{1F44B}\u{1F3FB} Welcome</h2>";
    let result = emoji_as_icons(&ctx(html));
    assert!(
        !result.fired,
        "2 rendered emoji with skin tones must count as 2: {}",
        result.detail
    );
}

#[test]
fn three_column_fires_only_when_grid_plus_emoji_headings_co_occur() {
    let html_with_emojis = r#"
            <section class="grid grid-cols-3 gap-8">
                <div><span>🚀</span><h3>Feature One</h3><p>Description.</p></div>
                <div><span>⚡</span><h3>Feature Two</h3><p>Description.</p></div>
                <div><span>✨</span><h3>Feature Three</h3><p>Description.</p></div>
                <div><span>🎉</span><h3>Feature Four</h3><p>Description.</p></div>
            </section>
        "#;
    let result = three_column_grid(&ctx(html_with_emojis));
    assert!(
        result.fired,
        "Should fire when grid + 3+ emoji-headings + 3+ cards co-occur"
    );
}

#[test]
fn three_column_clear_for_plain_grid_without_emojis() {
    let html = r#"
            <div class="grid grid-cols-3 gap-8">
                <div><h3>Feature One</h3><p>Description of feature one.</p></div>
                <div><h3>Feature Two</h3><p>Description of feature two.</p></div>
                <div><h3>Feature Three</h3><p>Description of feature three.</p></div>
            </div>
        "#;
    let result = three_column_grid(&ctx(html));
    assert!(
        !result.fired,
        "Plain 3-column grid (no emoji headings) is a 20-year-old pattern - must not fire"
    );
}

#[test]
fn three_column_clear_without_pattern() {
    let html = "<div><h2>About</h2><p>Some text.</p></div>";
    let result = three_column_grid(&ctx(html));
    assert!(!result.fired, "Should not fire without the grid pattern");
}
