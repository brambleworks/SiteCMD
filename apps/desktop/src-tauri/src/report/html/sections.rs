use super::super::{format_report_timestamp, ReportData};
use super::utils::{html_escape, score_color, severity_color};
use crate::checks::Severity;

pub(super) fn render_code_scan_section(data: &ReportData, pc: &str) -> String {
    if !data.sections.code_scan {
        return String::new();
    }
    let Some(code_scan) = data.code_scan.as_ref() else {
        return String::new();
    };

    let trend_text = match code_scan.previous_score {
        Some(previous) if code_scan.current_score > previous => {
            format!(
                "↑ {} from previous Code Scan",
                code_scan.current_score - previous
            )
        }
        Some(previous) if code_scan.current_score < previous => {
            format!(
                "↓ {} from previous Code Scan",
                previous - code_scan.current_score
            )
        }
        Some(_) => "→ stable from previous Code Scan".to_string(),
        None => "First Code Scan in this report window".to_string(),
    };
    let domain_trend_text = code_scan
        .domain_trend
        .as_ref()
        .map(|label| format!(" · {}", label))
        .unwrap_or_default();
    let checked_at = format_report_timestamp(&code_scan.checked_at);
    let domain_rows: String = code_scan
        .domains
        .iter()
        .map(|domain| {
            format!(
                r#"<tr>
      <td style="padding:6px 12px;border-bottom:1px solid #f3f4f6;font-size:12px">{name}</td>
      <td style="padding:6px 12px;border-bottom:1px solid #f3f4f6;font-size:12px;text-align:right;font-variant-numeric:tabular-nums">{count}</td>
    </tr>"#,
                name = html_escape(&domain.name),
                count = domain.issue_count,
            )
        })
        .collect();
    let code_issue_rows: String = code_scan
        .top_issues
        .iter()
        .map(|issue| {
            format!(
                r#"<tr>
      <td style="padding:6px 12px;border-bottom:1px solid #f3f4f6;font-size:12px">
        <span style="color:{sev_color};font-weight:700;text-transform:uppercase;font-size:10px">{severity}</span>
      </td>
      <td style="padding:6px 12px;border-bottom:1px solid #f3f4f6;font-size:12px">{category}</td>
      <td style="padding:6px 12px;border-bottom:1px solid #f3f4f6;font-size:12px;font-weight:500">{title}</td>
    </tr>"#,
                sev_color = severity_color(issue.severity),
                severity = issue.severity.as_str(),
                category = html_escape(&issue.category),
                title = html_escape(&issue.title),
            )
        })
        .collect();

    format!(
        r#"
    <div style="page-break-before:always"></div>
    <h2 style="font-size:20px;margin:32px 0 16px;color:#111827;border-bottom:2px solid {pc};padding-bottom:8px">Code Scan</h2>
    <p style="font-size:13px;color:#6b7280;margin-bottom:16px">Inside-out linked-project audit coverage for database, AI Safety, Security, Architecture, Operations, and Dependencies risks.</p>
    <div style="display:grid;grid-template-columns:repeat(4,1fr);gap:12px;margin-bottom:20px">
      <div style="background:#eff6ff;border-radius:8px;padding:12px;text-align:center">
        <div style="font-size:10px;text-transform:uppercase;letter-spacing:0.05em;color:#2563eb;font-weight:600">Diagnostic Score</div>
        <div style="font-size:22px;font-weight:700;color:{score_color};font-variant-numeric:tabular-nums">{score}</div>
      </div>
      <div style="background:#fef2f2;border-radius:8px;padding:12px;text-align:center">
        <div style="font-size:10px;text-transform:uppercase;letter-spacing:0.05em;color:{crit_color};font-weight:600">Critical</div>
        <div style="font-size:22px;font-weight:700;color:{crit_color};font-variant-numeric:tabular-nums">{critical}</div>
      </div>
      <div style="background:#fff7ed;border-radius:8px;padding:12px;text-align:center">
        <div style="font-size:10px;text-transform:uppercase;letter-spacing:0.05em;color:{high_color};font-weight:600">High</div>
        <div style="font-size:22px;font-weight:700;color:{high_color};font-variant-numeric:tabular-nums">{high}</div>
      </div>
      <div style="background:#f9fafb;border-radius:8px;padding:12px;text-align:center">
        <div style="font-size:10px;text-transform:uppercase;letter-spacing:0.05em;color:#6b7280;font-weight:600">Leading Domain</div>
        <div style="font-size:14px;font-weight:700;color:#111827;line-height:1.2">{top_domain}</div>
      </div>
    </div>
    <div style="display:flex;justify-content:space-between;gap:16px;margin-bottom:20px;font-size:12px;color:#6b7280">
      <div>{trend_text}{domain_trend_text}</div>
      <div>{issue_count} total code issues · checked {checked_at}{framework}</div>
    </div>
    <div style="display:grid;grid-template-columns:280px 1fr;gap:24px">
      <div>
        <h3 style="font-size:14px;font-weight:600;margin-bottom:8px">Code Domains</h3>
        <table style="width:100%;border-collapse:collapse">
          <thead><tr style="background:#f9fafb">
            <th style="padding:6px 12px;text-align:left;font-size:10px;text-transform:uppercase;color:#6b7280">Domain</th>
            <th style="padding:6px 12px;text-align:right;font-size:10px;text-transform:uppercase;color:#6b7280">Issues</th>
          </tr></thead>
          <tbody>{domain_rows}</tbody>
        </table>
      </div>
      <div>
        <h3 style="font-size:14px;font-weight:600;margin-bottom:8px">Top Code Issues</h3>
        {code_issues_table}
      </div>
    </div>"#,
        pc = pc,
        score = code_scan.current_score,
        score_color = score_color(code_scan.current_score),
        crit_color = severity_color(Severity::Critical),
        high_color = severity_color(Severity::High),
        critical = code_scan.critical_count,
        high = code_scan.high_count,
        top_domain = html_escape(
            &code_scan
                .top_domain
                .clone()
                .unwrap_or_else(|| "Code Scan".to_string()),
        ),
        trend_text = html_escape(&trend_text),
        domain_trend_text = html_escape(&domain_trend_text),
        issue_count = code_scan.issue_count,
        checked_at = html_escape(&checked_at),
        framework = code_scan
            .framework
            .as_ref()
            .map(|framework| format!(" · {}", html_escape(framework)))
            .unwrap_or_default(),
        domain_rows = domain_rows,
        code_issues_table = if code_issue_rows.is_empty() {
            r#"<p style="font-size:12px;color:#6b7280">No critical or high Code Scan issues were present in the latest run.</p>"#
                .to_string()
        } else {
            format!(
                r#"<table style="width:100%;border-collapse:collapse">
  <thead><tr style="background:#f9fafb">
    <th style="padding:6px 12px;text-align:left;font-size:10px;text-transform:uppercase;color:#6b7280">Severity</th>
    <th style="padding:6px 12px;text-align:left;font-size:10px;text-transform:uppercase;color:#6b7280">Domain</th>
    <th style="padding:6px 12px;text-align:left;font-size:10px;text-transform:uppercase;color:#6b7280">Issue</th>
  </tr></thead>
  <tbody>{code_issue_rows}</tbody>
</table>"#
            )
        },
    )
}

pub(super) fn render_issues_section(data: &ReportData, pc: &str) -> String {
    if !data.sections.top_issues || data.top_issues.is_empty() {
        return String::new();
    }

    let issue_rows: String = data
        .top_issues
        .iter()
        .map(|issue| {
            format!(
                r#"
        <tr>
          <td style="padding:8px 12px;border-bottom:1px solid #e5e7eb;font-size:12px">
            <span style="color:{sev_color};font-weight:700;text-transform:uppercase;font-size:10px">{severity}</span>
          </td>
          <td style="padding:8px 12px;border-bottom:1px solid #e5e7eb;font-size:12px">{category}</td>
          <td style="padding:8px 12px;border-bottom:1px solid #e5e7eb;font-size:12px;font-weight:500">{title}</td>
        </tr>"#,
                sev_color = severity_color(issue.severity),
                severity = issue.severity.as_str(),
                category = html_escape(&issue.category),
                title = html_escape(&issue.title),
            )
        })
        .collect();

    format!(
        r#"
    <div style="page-break-before:always"></div>
    <h2 style="font-size:20px;margin:32px 0 16px;color:#111827;border-bottom:2px solid {pc};padding-bottom:8px">{issues_title}</h2>
    <table style="width:100%;border-collapse:collapse;margin-bottom:32px">
      <thead><tr style="background:#f9fafb">
        <th style="padding:8px 12px;text-align:left;font-size:10px;text-transform:uppercase;color:#6b7280">Severity</th>
        <th style="padding:8px 12px;text-align:left;font-size:10px;text-transform:uppercase;color:#6b7280">Category</th>
        <th style="padding:8px 12px;text-align:left;font-size:10px;text-transform:uppercase;color:#6b7280">Issue</th>
      </tr></thead>
      <tbody>{issue_rows}</tbody>
    </table>"#,
        pc = pc,
        issues_title = if data.code_scan.is_some() {
            "Top Site Issues"
        } else {
            "Top Issues"
        },
        issue_rows = issue_rows,
    )
}
