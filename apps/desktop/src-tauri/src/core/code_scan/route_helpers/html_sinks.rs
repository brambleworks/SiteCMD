use super::*;

/// React's raw-HTML prop, the only sink the JSON-LD exemption below can excuse.
const REACT_RAW_HTML_SINK: &str = "dangerouslySetInnerHTML";

/// How far the JSON-LD exemption looks around a sink, in bytes.
const JSON_LD_SINK_WINDOW: usize = 200;

/// A `<script type="application/ld+json">` element whose
/// `REACT_RAW_HTML_SINK` prop carries `{{ __html: JSON.stringify(...) }}` is the
/// documented Next.js JSON-LD pattern: the sink receives serialized JSON inside
/// a non-executing script type, not markup.
/// True only when every raw-HTML sink in the file is that serialization inside
/// its own `application/ld+json` element. `JSON.stringify` escapes quotes and
/// backslashes but not `<`, so the identical serialization in any other element
/// is still a markup sink and keeps its finding even when a JSON-LD block sits
/// beside it in the same file.
pub(in crate::core::code_scan) fn is_json_ld_serialization_sink(content: &str) -> bool {
    if !content.contains(REACT_RAW_HTML_SINK) {
        return false;
    }
    if has_any_unquoted(
        &content.replace(REACT_RAW_HTML_SINK, ""),
        &DANGEROUS_HTML_PATTERNS,
    ) {
        return false;
    }
    content
        .match_indices(REACT_RAW_HTML_SINK)
        .all(|(start, _)| is_serialized_json_ld_sink_at(content, start))
}

/// Whether the single sink at `start` is a `JSON.stringify` value inside an
/// element that declares `type="application/ld+json"`.
fn is_serialized_json_ld_sink_at(content: &str, start: usize) -> bool {
    let mut end = (start + JSON_LD_SINK_WINDOW).min(content.len());
    while !content.is_char_boundary(end) {
        end += 1;
    }
    JSON_LD_SERIALIZED_SINK_PATTERN.is_match(&content[start..end])
        && enclosing_element_declares_json_ld(content, start)
}

/// Whether the element or `createElement` call that owns the sink at `start`
/// declares the JSON-LD script type. The look-back starts at that element's own
/// opening marker, never crosses an earlier sink, and never crosses an element
/// boundary, so neither a JSON-LD block elsewhere in the file nor a
/// self-closing `<script type="application/ld+json" />` can vouch for a sink in
/// a different element. An element whose opening marker is not found is not
/// excused.
///
/// Two safe shapes stay over-reported on purpose and keep `unsafe-html`: a
/// `type` attribute written after the sink on the same element, and a sink
/// pushed more than `JSON_LD_SINK_WINDOW` bytes past its `<` by other
/// attributes. A tag whose attributes contain `>` (an inline arrow function,
/// say) also stays reported, because that `>` is indistinguishable from the end
/// of the tag here.
fn enclosing_element_declares_json_ld(content: &str, start: usize) -> bool {
    const JSON_LD_TYPE: &str = "application/ld+json";
    let head = &content[..start];
    let Some(opening) = [head.rfind('<'), head.rfind("createElement")]
        .into_iter()
        .flatten()
        .max()
    else {
        return false;
    };
    let floor = head
        .rfind(REACT_RAW_HTML_SINK)
        .map_or(0, |index| index + REACT_RAW_HTML_SINK.len())
        .max(start.saturating_sub(JSON_LD_SINK_WINDOW));
    if opening < floor {
        return false;
    }
    let enclosing = &head[opening..];
    // An element that already closed cannot own this sink.
    if enclosing.starts_with('<') && enclosing.contains('>') {
        return false;
    }
    enclosing.contains(JSON_LD_TYPE)
}
