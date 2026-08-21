//! JSON-LD extraction and node flattening for structured-data checks.

use serde_json::Value;

/// A JSON-LD block that failed to parse.
pub struct ParseFailure {
    /// Zero-based index of the JSON-LD block in document order.
    pub block_index: usize,
    pub line: usize,
    pub column: usize,
    /// serde_json's message, which already names the line and column.
    pub message: String,
}

/// Outcome of extracting and parsing every JSON-LD block on the page.
pub struct JsonLdExtraction {
    pub block_count: usize,
    pub nodes: Vec<Value>,
    pub failures: Vec<ParseFailure>,
}

/// Extract JSON-LD blocks using a same-length ASCII-lowercased search copy.
pub fn extract_blocks<'a>(body: &'a str, body_lower: &str) -> Vec<&'a str> {
    let mut blocks = Vec::new();
    let mut cursor = 0;
    while let Some(rel) = body_lower[cursor..].find("<script") {
        let tag_start = cursor + rel;
        let name_end = tag_start + "<script".len();
        if !matches!(
            body_lower[name_end..].chars().next(),
            Some(' ' | '\t' | '\n' | '\r' | '/' | '>')
        ) {
            cursor = name_end;
            continue;
        }
        let Some(tag_end_rel) = body_lower[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + tag_end_rel;
        let Some(content_end) = find_script_close(body_lower, tag_end + 1) else {
            break;
        };
        let opening_tag = &body[tag_start..=tag_end];
        let is_json_ld = crate::checks::html_attrs::attr_value(opening_tag, "type")
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/ld+json"));
        if is_json_ld {
            blocks.push(&body[tag_end + 1..content_end]);
        }
        cursor = content_end;
    }
    blocks
}

/// Find an actual HTML `</script>` closing-tag start rather than accepting a
/// longer tag name such as `</scripture>`. Whitespace before `>` is permitted.
fn find_script_close(body_lower: &str, from: usize) -> Option<usize> {
    let mut cursor = from;
    while let Some(rel) = body_lower[cursor..].find("</script") {
        let candidate = cursor + rel;
        let name_end = candidate + "</script".len();
        if matches!(
            body_lower[name_end..].chars().next(),
            Some(' ' | '\t' | '\n' | '\r' | '>')
        ) {
            return Some(candidate);
        }
        cursor = name_end;
    }
    None
}

/// Parse every extracted block, collecting validatable nodes and failures.
pub fn parse_blocks(blocks: &[&str]) -> JsonLdExtraction {
    let mut nodes = Vec::new();
    let mut failures = Vec::new();
    for (block_index, raw) in blocks.iter().enumerate() {
        match serde_json::from_str::<Value>(raw) {
            Ok(value) => collect_nodes(value, &mut nodes),
            Err(err) => failures.push(ParseFailure {
                block_index,
                line: err.line(),
                column: err.column(),
                message: err.to_string(),
            }),
        }
    }
    JsonLdExtraction {
        block_count: blocks.len(),
        nodes,
        failures,
    }
}

/// Flatten a parsed JSON-LD document into validatable nodes. Top-level arrays
/// contribute each element; `@graph` containers contribute each graph node,
/// plus the container itself when it carries its own properties.
fn collect_nodes(value: Value, nodes: &mut Vec<Value>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_nodes(item, nodes);
            }
        }
        Value::Object(mut obj) => {
            if let Some(graph) = obj.remove("@graph") {
                collect_nodes(graph, nodes);
            }
            if !obj.is_empty() {
                nodes.push(Value::Object(obj));
            }
        }
        _ => {}
    }
}
