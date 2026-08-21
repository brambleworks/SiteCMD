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
fn div_soup_fires_with_mostly_divs() {
    let html = "<div>1</div><div>2</div><div>3</div><div>4</div><div>5</div>\
                     <div>6</div><div>7</div><div>8</div><div>9</div><main>10</main>";
    let result = div_soup_ratio(&ctx(html));
    assert!(result.fired, "Should fire at 90% div ratio");
}

#[test]
fn div_soup_clear_with_semantic_html() {
    let html = "<main><section><article>A</article></section><nav>N</nav>\
                     <aside>S</aside><header>H</header><footer>F</footer><div>D</div></main>";
    let result = div_soup_ratio(&ctx(html));
    assert!(
        !result.fired,
        "Should not fire with good semantic structure"
    );
}

#[test]
fn div_soup_clear_with_too_few_elements() {
    let html = "<div>A</div><div>B</div>";
    let result = div_soup_ratio(&ctx(html));
    assert!(!result.fired, "Should not fire with <5 elements");
}

#[test]
fn heading_hierarchy_fires_on_level_skip() {
    let html = "<h1>Title</h1><h4>Subsection</h4>";
    let result = heading_hierarchy(&ctx(html));
    assert!(result.fired, "Should fire when skipping h2-h3");
}

#[test]
fn heading_hierarchy_fires_on_multiple_h1() {
    let html = "<h1>First</h1><h2>Sub</h2><h1>Second</h1>";
    let result = heading_hierarchy(&ctx(html));
    assert!(result.fired, "Should fire with multiple h1 tags");
    assert!(result.detail.contains("confirm"));
    assert!(!result.detail.contains("should be exactly 1"));
}

#[test]
fn heading_hierarchy_describes_missing_h1_as_a_contextual_review() {
    let result = heading_hierarchy(&ctx("<h2>Section</h2><h3>Details</h3>"));
    assert!(result.fired);
    assert!(result.detail.contains("fetched HTML"));
    assert!(result.detail.contains("confirm"));
    assert!(!result.detail.contains("violation"));
}

#[test]
fn heading_hierarchy_clear_with_proper_structure() {
    let html = "<h1>Title</h1><h2>Section</h2><h3>Subsection</h3><h2>Another</h2>";
    let result = heading_hierarchy(&ctx(html));
    assert!(
        !result.fired,
        "Should not fire with proper heading hierarchy"
    );
}

#[test]
fn heading_hierarchy_ignores_script_templates_and_comments() {
    let html = r#"<h1>Title</h1><h2>Section</h2>
        <script type="text/x-template"><h1>Template title</h1><h5>Deep</h5></script>
        <!-- <h1>Old heading</h1> -->"#;
    let result = heading_hierarchy(&ctx(html));
    assert!(
        !result.fired,
        "script templates and comments are not page headings: {}",
        result.detail
    );
}

#[test]
fn form_accessibility_fires_without_labels() {
    let html = r#"<input type="text" id="name"><input type="email" id="email">"#;
    let result = form_accessibility(&ctx(html));
    assert!(result.fired, "Should fire when inputs have no labels");
}

#[test]
fn form_accessibility_clear_with_labels() {
    let html = r#"
            <label for="name">Name</label><input type="text" id="name">
            <label for="email">Email</label><input type="email" id="email">
        "#;
    let result = form_accessibility(&ctx(html));
    assert!(
        !result.fired,
        "Should not fire when inputs have matching labels"
    );
}

#[test]
fn form_accessibility_skips_hidden_and_submit() {
    let html = r#"<input type="hidden" name="token"><input type="submit" value="Go">"#;
    let result = form_accessibility(&ctx(html));
    assert!(!result.fired, "Should skip hidden and submit inputs");
}

#[test]
fn form_accessibility_credits_wrapping_labels() {
    let html = r#"
        <label>Name <input type="text"></label>
        <label><span>Email</span> <input type="email"></label>
    "#;
    let result = form_accessibility(&ctx(html));
    assert!(
        !result.fired,
        "wrapped inputs are labeled: {}",
        result.detail
    );
}

#[test]
fn form_accessibility_reads_unquoted_attributes() {
    let html = "<label for=name>Name</label><input id=name type=text><input type=hidden name=csrf>";
    let result = form_accessibility(&ctx(html));
    assert!(
        !result.fired,
        "minified unquoted attributes must parse: {}",
        result.detail
    );
}

#[test]
fn clickable_div_fires_with_onclick_on_div() {
    let html = r#"<div onclick="doSomething()">Click me</div>"#;
    let result = button_vs_clickable_div(&ctx(html));
    assert!(result.fired, "Should fire when div has onclick");
}

#[test]
fn clickable_div_clear_with_button_onclick() {
    let html = r#"<button onclick="doSomething()">Click me</button>"#;
    let result = button_vs_clickable_div(&ctx(html));
    assert!(!result.fired, "Should not fire when button has onclick");
}

#[test]
fn clickable_div_clear_with_role_button_retrofit() {
    let html = r#"<div onclick="go()" role="button" tabindex="0">Click me</div>
        <span role="button">Toggle</span>"#;
    let result = button_vs_clickable_div(&ctx(html));
    assert!(
        !result.fired,
        "role=button exempts the element: {}",
        result.detail
    );
}

#[test]
fn clickable_div_ignores_data_onclick_attribute() {
    let html = r#"<div data-onclick="legacy-hook">Static</div>"#;
    let result = button_vs_clickable_div(&ctx(html));
    assert!(!result.fired, "data-onclick is not a click handler");
}

#[test]
fn clickable_div_fires_when_role_is_elsewhere() {
    // The exemption must come from the same tag, not a neighboring one.
    let html = r#"<span role="button">ok</span><div onclick="go()">Click me</div>"#;
    let result = button_vs_clickable_div(&ctx(html));
    assert!(result.fired, "onclick div without role=button still fires");
}

#[test]
fn missing_lang_fires_when_absent() {
    let html = "<html><head></head><body></body></html>";
    let result = missing_lang(&ctx(html));
    assert!(result.fired, "Should fire when lang is missing");
}

#[test]
fn missing_lang_clear_when_present() {
    let html = r#"<html lang="en"><head></head><body></body></html>"#;
    let result = missing_lang(&ctx(html));
    assert!(!result.fired, "Should not fire when lang is present");
}
