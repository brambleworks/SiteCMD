use super::super::{DailyPoint, ScorePoint};
use crate::checks::Severity;

// Keep standalone report bands aligned with frontend tokens; guardrails pin both.
pub(super) fn score_color(score: u32) -> &'static str {
    if score >= 90 {
        "oklch(0.627 0.194 149.57)" // excellent
    } else if score >= 70 {
        "oklch(0.705 0.213 47.604)" // good
    } else if score >= 50 {
        "oklch(0.769 0.188 70.08)" // attention
    } else if score >= 30 {
        "oklch(0.645 0.246 16.44)" // poor
    } else {
        "oklch(0.577 0.245 27.33)" // critical
    }
}

pub(super) fn severity_color(sev: Severity) -> &'static str {
    match sev {
        Severity::Critical => "oklch(0.577 0.245 27.33)",
        Severity::High => "oklch(0.645 0.246 16.44)",
        Severity::Medium => "oklch(0.705 0.213 47.604)",
        Severity::Low => "oklch(0.627 0.194 149.57)",
    }
}

pub(super) fn svg_score_trend(points: &[ScorePoint], color: &str, w: u32, h: u32) -> String {
    if points.len() < 2 {
        return String::new();
    }
    let min = points
        .iter()
        .map(|p| p.score)
        .min()
        .unwrap_or(0)
        .saturating_sub(5);
    let max = points.iter().map(|p| p.score).max().unwrap_or(100) + 5;
    let range = (max - min).max(1) as f64;
    let step = w as f64 / (points.len() - 1).max(1) as f64;

    let path: Vec<String> = points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let x = i as f64 * step;
            let y = h as f64 - ((p.score - min) as f64 / range * h as f64);
            if i == 0 {
                format!("M{:.1},{:.1}", x, y)
            } else {
                format!("L{:.1},{:.1}", x, y)
            }
        })
        .collect();

    format!(
        r#"<svg viewBox="0 0 {w} {h}" style="width:100%;height:{h}px">
          <path d="{}" fill="none" stroke="{color}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>"#,
        path.join(" "),
    )
}

pub(super) fn svg_visitor_chart(points: &[DailyPoint], color: &str, w: u32, h: u32) -> String {
    if points.is_empty() {
        return String::new();
    }
    let max_val = points.iter().map(|p| p.value).max().unwrap_or(1).max(1) as f64;
    let bar_w = (w as f64 / points.len() as f64 * 0.7).max(2.0);
    let gap = w as f64 / points.len() as f64;

    let bars: Vec<String> = points.iter().enumerate().map(|(i, p)| {
        let bar_h = (p.value as f64 / max_val * h as f64).max(1.0);
        let x = i as f64 * gap + (gap - bar_w) / 2.0;
        let y = h as f64 - bar_h;
        format!(r#"<rect x="{x:.1}" y="{y:.1}" width="{bar_w:.1}" height="{bar_h:.1}" fill="{color}" rx="1"/>"#)
    }).collect();

    format!(
        r#"<svg viewBox="0 0 {w} {h}" style="width:100%;height:{h}px">{}</svg>"#,
        bars.join("\n"),
    )
}

pub(super) fn sanitize_css_color(color: &str) -> String {
    let c = color.trim();
    if c.starts_with('#')
        && c[1..].chars().all(|ch| ch.is_ascii_hexdigit())
        && (c.len() == 4 || c.len() == 7 || c.len() == 9)
    {
        return c.to_string();
    }
    let lower = c.to_lowercase();
    let is_css_fn = lower.starts_with("rgb(")
        || lower.starts_with("rgba(")
        || lower.starts_with("hsl(")
        || lower.starts_with("hsla(")
        || lower.starts_with("oklch(")
        || lower.starts_with("lch(");
    if is_css_fn
        && c.chars()
            .all(|ch| ch.is_alphanumeric() || matches!(ch, '(' | ')' | ',' | '.' | '%' | ' '))
    {
        return c.to_string();
    }
    if c.chars().all(|ch| ch.is_ascii_alphabetic()) && c.len() <= 20 {
        return c.to_string();
    }
    "#2563eb".to_string()
}

pub(crate) fn sanitize_logo_data_url(value: &str) -> Option<String> {
    const MAX_LOGO_DATA_URL_LEN: usize = 5_000_000;
    if value.len() > MAX_LOGO_DATA_URL_LEN {
        return None;
    }

    let allowed_prefixes = [
        "data:image/png;base64,",
        "data:image/jpeg;base64,",
        "data:image/webp;base64,",
        "data:image/gif;base64,",
    ];
    if !allowed_prefixes
        .iter()
        .any(|prefix| value.starts_with(prefix))
    {
        return None;
    }

    let (_, base64_payload) = value.split_once(',')?;
    if !base64_payload
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '/' | '='))
    {
        return None;
    }

    Some(value.to_string())
}

pub(super) fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::{score_color, severity_color};
    use crate::checks::Severity;

    #[test]
    fn score_color_uses_the_five_app_bands_from_lib_score_ts() {
        assert_eq!(score_color(100), score_color(90));
        assert_eq!(score_color(89), score_color(70));
        assert_eq!(score_color(69), score_color(50));
        assert_eq!(score_color(49), score_color(30));
        assert_eq!(score_color(29), score_color(0));
        let bands = [
            score_color(90),
            score_color(70),
            score_color(50),
            score_color(30),
            score_color(0),
        ];
        for (i, band) in bands.iter().enumerate() {
            for other in &bands[i + 1..] {
                assert_ne!(band, other, "each score band must render a distinct color");
            }
        }
    }

    #[test]
    fn severity_color_maps_each_severity_to_a_distinct_token_value() {
        let colors = [
            severity_color(Severity::Critical),
            severity_color(Severity::High),
            severity_color(Severity::Medium),
            severity_color(Severity::Low),
        ];
        for (i, color) in colors.iter().enumerate() {
            for other in &colors[i + 1..] {
                assert_ne!(color, other);
            }
        }
    }
}
