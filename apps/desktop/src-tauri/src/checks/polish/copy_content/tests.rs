use super::*;

fn ctx(html: &str) -> PolishContext {
    ctx_at("https://example.com", html)
}

fn ctx_at(url: &str, html: &str) -> PolishContext {
    PolishContext {
        url: url::Url::parse(url).unwrap(),
        html: html.to_string(),
        css: String::new(),
        html_lower_cache: std::sync::OnceLock::new(),
    }
}

#[test]
fn buzzwords_exempt_a_dictionary_word_that_names_the_site() {
    let html = format!(
        "<title>Smart Kettle Guides | SmartKettle</title>\
         <meta content=\"SmartKettle\" property=\"og:site_name\">\
         <h1>Smart kettles, smart plugs, smart hubs</h1><p>{}</p>",
        "The smart kettle is a smart buy. ".repeat(15)
    );
    let branded = ai_buzzword_dictionary(&ctx_at("https://smartkettle.com/", &html));
    assert!(!branded.fired, "{}", branded.detail);
    assert_eq!(
        branded.data["brand_words_excluded"],
        serde_json::json!(["smart"])
    );

    let unbranded = ai_buzzword_dictionary(&ctx_at(
        "https://example.com/",
        &html.replace("SmartKettle", "Example"),
    ));
    assert!(unbranded.fired, "{}", unbranded.detail);
    assert_eq!(
        unbranded.data["brand_words_excluded"],
        serde_json::json!([])
    );
}

#[test]
fn the_domain_label_alone_exempts_a_dictionary_word_it_contains() {
    let html = format!(
        "<h1>Sleek desks, sleek chairs, sleek lamps</h1><p>{}</p>",
        "A sleek desk for a sleek room. ".repeat(15)
    );
    let result = ai_buzzword_dictionary(&ctx_at("https://www.sleekfurniture.co.uk/shop", &html));
    assert!(!result.fired, "{}", result.detail);
    assert_eq!(
        result.data["brand_words_excluded"],
        serde_json::json!(["sleek"])
    );
}

#[test]
fn a_buzzword_laden_title_does_not_exempt_its_own_copy() {
    let html = r#"
            <title>Seamlessly Elevate Your Workflow | Acme</title>
            <meta property="og:site_name" content="Acme">
            <h1>Seamlessly Elevate Your Workflow</h1>
            <p>Our cutting-edge platform empowers you to effortlessly harness the power
            of innovative solutions. Leverage our comprehensive, state-of-the-art tools
            to revolutionize your dynamic approach. Unlock unparalleled scalable synergy
            and supercharge your next-generation journey.</p>
        "#;
    let result = ai_buzzword_dictionary(&ctx_at("https://acme.com/", html));
    assert!(result.fired, "{}", result.detail);
    assert!(
        result.data["top_words"]
            .as_array()
            .expect("top_words array")
            .iter()
            .any(|w| w["word"].as_str() == Some("seamlessly")),
        "title copy must still count: {}",
        result.detail
    );
    assert_eq!(result.data["brand_words_excluded"], serde_json::json!([]));
}

#[test]
fn the_exclusion_list_names_only_words_the_page_actually_uses() {
    let page = |copy: &str| {
        format!(
            "<title>Cabinet Fitting Guides | ModernKitchen</title>\
             <meta property=\"og:site_name\" content=\"ModernKitchen\">\
             <h1>Cabinet fitting guides</h1><p>{copy}</p>"
        )
    };

    // "modern" is in the brand, but this page never writes it, so reporting it
    // as excluded would claim a suppression that never happened.
    let unused = ai_buzzword_dictionary(&ctx_at(
        "https://modernkitchen.com/",
        &page("Measure twice, cut once."),
    ));
    assert!(!unused.fired, "{}", unused.detail);
    assert!(
        unused.data.get("brand_words_excluded").is_none(),
        "no word was suppressed: {}",
        unused.data
    );

    // The same brand on a page that does use the word reports the suppression.
    let used = ai_buzzword_dictionary(&ctx_at(
        "https://modernkitchen.com/",
        &page("A modern kitchen, measured twice."),
    ));
    assert!(!used.fired, "{}", used.detail);
    assert_eq!(
        used.data["brand_words_excluded"],
        serde_json::json!(["modern"])
    );
}

#[test]
fn og_site_name_is_read_from_either_the_property_or_name_attribute() {
    // Open Graph specifies `property`, but plenty of templates emit `name`.
    let body = format!(
        "<h1>Sleek desks, sleek chairs, sleek lamps</h1><p>{}</p>",
        "A sleek desk for a sleek room. ".repeat(15)
    );
    for attribute in ["property", "name"] {
        let html = format!("<meta {attribute}=\"og:site_name\" content=\"Sleek Supply\">{body}");
        // example.com cannot supply the brand, so only the meta tag can.
        let result = ai_buzzword_dictionary(&ctx_at("https://example.com/", &html));
        assert!(!result.fired, "{attribute}: {}", result.detail);
        assert_eq!(
            result.data["brand_words_excluded"],
            serde_json::json!(["sleek"]),
            "{attribute}"
        );
    }

    let unnamed = ai_buzzword_dictionary(&ctx_at("https://example.com/", &body));
    assert!(unnamed.fired, "{}", unnamed.detail);
}

#[test]
fn brand_words_split_camel_case_and_punctuation() {
    assert_eq!(brand_words("SmartHomeU"), vec!["smart", "home", "u"]);
    assert_eq!(brand_words("SiteCMD"), vec!["site", "cmd"]);
    assert_eq!(
        brand_words("Cutting Edge Tools, Inc."),
        vec!["cutting", "edge", "tools", "inc"]
    );
    assert_eq!(alphanumeric_key("Smart Home U"), "smarthomeu");
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

#[test]
fn html_comments_are_not_prose() {
    // The polish fixture declares its defects in a leading comment whose text
    // contains a `>`; the tag stripper leaked the tail into the counted copy.
    let comment = "<!-- FIXTURE polish. Declared defects: inclusive framing (whether you're a \
                   beginner), rounded-2xl and shadow-purple-500 classes; ratio > 0.5 -->";
    let body = "<main><p>Whether you're a plumber or a poet, the kettle boils.</p></main>";

    let with_comment = inclusive_framing(&ctx(&format!("{comment}{body}")));
    let without_comment = inclusive_framing(&ctx(body));
    assert_eq!(
        with_comment.data["occurrences"], without_comment.data["occurrences"],
        "a comment must not add an occurrence"
    );
    assert_eq!(with_comment.data["occurrences"], 1);
}

#[test]
fn a_script_or_style_body_is_not_prose() {
    let noisy = "<script>const copy = \"whether you're a pro\";</script>\
                 <style>.a { content: \"whether you're a pro\"; }</style>\
                 <main><p>Plain copy.</p></main>";
    assert!(!inclusive_framing(&ctx(noisy)).fired);
}

#[test]
fn buzzword_raw_data_reports_the_weight_and_the_tier_separately() {
    let html = "<h1>Seamlessly elevate and harness your workflow</h1>\
        <p>Seamlessly elevate the harness. Seamlessly elevate and harness everything, \
        because you can seamlessly elevate and harness the harness you elevate.</p>";
    let result = ai_buzzword_dictionary(&ctx(html));
    assert!(result.fired, "{}", result.detail);
    let top = result.data["top_words"].as_array().expect("top words");
    let seamlessly = top
        .iter()
        .find(|entry| entry["word"] == "seamlessly")
        .expect("seamlessly counted");
    assert_eq!(seamlessly["weight"], 3, "tier 1 words score three points");
    assert_eq!(seamlessly["tier"], 1, "and belong to tier 1");
}

#[test]
fn three_column_grid_counts_an_emoji_in_the_last_card_heading() {
    // Fixture `grid-3in.html`: three cards, emoji inside each <h3>. The old
    // "emoji then a following heading" match never counted the last card.
    let html = r#"<main><h1>Grid</h1>
        <section class="grid grid-cols-3">
        <article><h3>&#x1F680; Fast</h3><p>Ships quickly.</p></article>
        <article><h3>&#x1F512; Secure</h3><p>Locks down.</p></article>
        <article><h3>&#x1F4E6; Packaged</h3><p>Boxed up.</p></article>
        </section></main>"#;
    let html = html.replace("&#x1F680;", "\u{1F680}");
    let html = html.replace("&#x1F512;", "\u{1F512}");
    let html = html.replace("&#x1F4E6;", "\u{1F4E6}");
    let result = three_column_grid(&ctx(&html));
    assert!(result.fired, "{}", result.detail);
    assert_eq!(result.data["emoji_heading_pairs"], 3);
}

#[test]
fn three_column_grid_still_counts_an_emoji_in_the_element_before_the_heading() {
    // Fixture `grid-3out.html`, which the old pattern did catch.
    let html = "<main><h1>Grid</h1>\
        <section class=\"grid grid-cols-3\">\
        <article><span>\u{1F680}</span><h3>Fast</h3><p>Ships quickly.</p></article>\
        <article><span>\u{1F512}</span><h3>Secure</h3><p>Locks down.</p></article>\
        <article><span>\u{1F4E6}</span><h3>Packaged</h3><p>Boxed up.</p></article>\
        </section></main>";
    let result = three_column_grid(&ctx(html));
    assert!(result.fired, "{}", result.detail);
    assert_eq!(result.data["emoji_heading_pairs"], 3);
}

#[test]
fn three_column_grid_counts_the_wider_emoji_class_shared_with_emoji_as_icons() {
    // U+2728 SPARKLES sits in the 2700-27BF block the old pattern omitted.
    let html = "<main><h1>Grid</h1>\
        <section class=\"grid grid-cols-3\">\
        <article><h3>\u{2728} Fast</h3><p>Ships quickly.</p></article>\
        <article><h3>\u{2714} Secure</h3><p>Locks down.</p></article>\
        <article><h3>\u{2705} Packaged</h3><p>Boxed up.</p></article>\
        </section></main>";
    let result = three_column_grid(&ctx(html));
    assert!(result.fired, "{}", result.detail);
    assert!(emoji_as_icons(&ctx(html)).fired, "the two must agree");
}

#[test]
fn ordinary_question_headlines_are_not_a_marketing_formula() {
    // BBC headlines. Question-form journalism is not an AI pattern.
    let html = "<h2>Why did Courteeners frontman buy tickets to his own gig?</h2>\
        <h2>Why are so many people leaving London?</h2>\
        <h3>Why is the sky blue tonight?</h3>";
    let result = ai_header_formulas(&ctx(html));
    assert!(!result.fired, "{}", result.detail);
}

#[test]
fn documented_formula_headlines_still_fire() {
    let html = "<h2>Why speed matters</h2>\
        <h2>The ultimate guide to shipping faster</h2>\
        <h3>7 simple ways to cut your bundle</h3>";
    let result = ai_header_formulas(&ctx(html));
    assert!(result.fired, "{}", result.detail);
    assert_eq!(result.data["matched_count"], 3);
}

#[test]
fn a_commented_out_card_section_is_not_a_three_column_grid() {
    // A whole feature section left in a comment: no grid container, no cards,
    // and no emoji headings are on the page, so nothing may fire.
    let cards = "<section class=\"grid grid-cols-3\">\
        <article><h3>\u{1F680} Fast</h3><p>Ships quickly.</p></article>\
        <article><h3>\u{1F512} Secure</h3><p>Locks down.</p></article>\
        <article><h3>\u{1F4E6} Packaged</h3><p>Boxed up.</p></article>\
        </section>";
    let commented = format!("<main><h1>Live page</h1><!-- {cards} --><p>Plain copy.</p></main>");

    assert!(
        !three_column_grid(&ctx(&commented)).fired,
        "a commented-out card section is not a rendered grid"
    );
    assert!(
        !emoji_as_icons(&ctx(&commented)).fired,
        "emoji inside a comment are not emoji on the page"
    );

    // The same markup outside the comment still fires, so the strip did not
    // disable the signals.
    let live = format!("<main><h1>Live page</h1>{cards}</main>");
    assert!(three_column_grid(&ctx(&live)).fired);
    assert!(emoji_as_icons(&ctx(&live)).fired);
}

#[test]
fn headings_inside_comments_and_scripts_are_not_headings() {
    // ai-header-formulas and the full-weight heading term of the buzzword
    // dictionary both read extract_heading_text.
    let hidden = "<main><h1>Live page</h1>\
        <!-- <h2>The ultimate guide to shipping</h2><h2>7 ways to elevate</h2> -->\
        <script type=\"text/template\"><h2>The future of seamlessly harnessing</h2></script>\
        <p>Plain copy with nothing to score.</p></main>";
    assert!(
        !ai_header_formulas(&ctx(hidden)).fired,
        "commented-out and templated headings are not page headings"
    );

    // Two live formula headings still fire.
    let live = "<main><h1>Live page</h1>\
        <h2>The ultimate guide to shipping</h2><h2>7 ways to elevate your stack</h2></main>";
    let result = ai_header_formulas(&ctx(live));
    assert!(result.fired, "{}", result.detail);
    assert_eq!(result.data["matched_count"], 2);
}
