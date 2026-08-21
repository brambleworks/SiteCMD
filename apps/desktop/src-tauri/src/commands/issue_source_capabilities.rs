use std::sync::Arc;

use tauri::AppHandle;

use crate::db::Database;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IssueVerifyStrategy {
    WebScan,
    CodeScan,
    IntegrationPoll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IssueSourceCapability {
    pub source: &'static str,
    pub verify: IssueVerifyStrategy,
}

const ISSUE_SOURCE_CAPABILITIES: &[IssueSourceCapability] = &[
    IssueSourceCapability {
        source: "web_scan",
        verify: IssueVerifyStrategy::WebScan,
    },
    IssueSourceCapability {
        source: "code_scan",
        verify: IssueVerifyStrategy::CodeScan,
    },
    IssueSourceCapability {
        source: "psi",
        verify: IssueVerifyStrategy::IntegrationPoll,
    },
    IssueSourceCapability {
        source: "gsc",
        verify: IssueVerifyStrategy::IntegrationPoll,
    },
    IssueSourceCapability {
        source: "updates",
        verify: IssueVerifyStrategy::IntegrationPoll,
    },
    IssueSourceCapability {
        source: "uptimerobot",
        verify: IssueVerifyStrategy::IntegrationPoll,
    },
];

pub(crate) fn issue_source_capability(source: &str) -> Option<IssueSourceCapability> {
    ISSUE_SOURCE_CAPABILITIES
        .iter()
        .copied()
        .find(|capability| capability.source == source)
}

async fn run_web_issue_verification(
    app: Option<&AppHandle>,
    db: Arc<Database>,
    project_id: i64,
    environment_url: &str,
    page_url: &str,
    check_ids: &[String],
) -> Result<(), crate::core::scanner::ScanError> {
    let scan_control = crate::commands::scan::ScanControlState::default();
    crate::commands::scan::verification::run_bounded_web_verification(
        app,
        db,
        &scan_control,
        Some(project_id),
        Some(environment_url.to_string()),
        page_url.to_string(),
        check_ids.to_vec(),
        None,
        None,
    )
    .await
    .map(|_| ())
}

pub(crate) async fn verify_issue_source(
    capability: IssueSourceCapability,
    app: &AppHandle,
    db: Arc<Database>,
    project_id: i64,
    env_url: &str,
    check_id: &str,
    web_scan_url: Option<&str>,
    web_check_ids: &[String],
) -> Result<(), String> {
    match capability.verify {
        IssueVerifyStrategy::WebScan => {
            let url = web_scan_url.unwrap_or(env_url).to_string();
            let requested = if web_check_ids.is_empty() {
                vec![check_id.to_string()]
            } else {
                web_check_ids.to_vec()
            };
            run_web_issue_verification(Some(app), db, project_id, env_url, &url, &requested)
                .await
                .map_err(|e| format!("{} verify: {:?}", capability.source, e))?;
        }
        IssueVerifyStrategy::CodeScan => {
            let scan_control = crate::commands::scan::ScanControlState::default();
            let action_key =
                crate::commands::scan::execution::generate_scan_action_key("verification-code")?;
            let project_path = db.get_project_path(project_id);
            let result = crate::commands::scan::execution::run_scan_execution_internal(
                app.clone(),
                db,
                &scan_control,
                crate::commands::scan::execution::RunScanExecutionRequest {
                    project_id: Some(project_id),
                    environment_id: None,
                    environment_url: Some(env_url.to_string()),
                    requested_mode: crate::core::scan_execution::ScanExecutionMode::Code,
                    web_focus: None,
                    urls: Vec::new(),
                    enabled_categories: None,
                    timeout_secs: None,
                    axe_enabled: None,
                    inspect_local_databases: false,
                    project_path,
                    scan_request_id: None,
                    retention: Some(crate::db::MAX_SCAN_RETENTION),
                    trigger: crate::core::scan_execution::ScanTrigger::Verification,
                    idempotency_key: action_key,
                },
            )
            .await
            .map_err(|e| format!("{} verify: {}", capability.source, e))?;
            if result.code_result.is_none() {
                return Err(format!(
                    "{} verify: {}",
                    capability.source,
                    result
                        .execution
                        .code_detail
                        .unwrap_or_else(|| "Code Scan produced no result".into())
                ));
            }
        }
        IssueVerifyStrategy::IntegrationPoll => {
            crate::core::integration_scheduler::request_immediate_poll(
                capability.source,
                project_id,
                Some(env_url),
            )
            .await
            .map_err(|e| format!("{} verify: {}", capability.source, e))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{issue_source_capability, run_web_issue_verification, IssueVerifyStrategy};
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn issue_source_registry_maps_known_sources() {
        assert_eq!(
            issue_source_capability("web_scan").map(|cap| cap.verify),
            Some(IssueVerifyStrategy::WebScan)
        );
        assert_eq!(
            issue_source_capability("code_scan").map(|cap| cap.verify),
            Some(IssueVerifyStrategy::CodeScan)
        );
        assert_eq!(
            issue_source_capability("updates").map(|cap| cap.verify),
            Some(IssueVerifyStrategy::IntegrationPoll)
        );
    }

    #[test]
    fn issue_source_registry_rejects_unknown_sources() {
        assert!(issue_source_capability("made_up").is_none());
    }

    #[tokio::test]
    async fn redirected_web_verification_keeps_only_the_effective_occurrence() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind redirect fixture");
        let base_url = format!("http://{}", listener.local_addr().expect("fixture address"));
        let authored_url = format!("{base_url}/docs");
        let effective_url = format!("{authored_url}/");
        let effective_for_server = effective_url.clone();
        let server = tokio::spawn(async move {
            loop {
                let (mut stream, _) = match listener.accept().await {
                    Ok(connection) => connection,
                    Err(_) => break,
                };
                let effective_url = effective_for_server.clone();
                tokio::spawn(async move {
                    let mut buffer = [0_u8; 4096];
                    let read = match stream.read(&mut buffer).await {
                        Ok(0) | Err(_) => return,
                        Ok(read) => read,
                    };
                    let request = String::from_utf8_lossy(&buffer[..read]);
                    let request_line = request.lines().next().unwrap_or_default();
                    let method = request_line.split_whitespace().next().unwrap_or("GET");
                    let path = request_line.split_whitespace().nth(1).unwrap_or("/");
                    let (status, location, body) = if path == "/docs" {
                        ("302 Found", format!("Location: {effective_url}\r\n"), "")
                    } else {
                        (
                            "200 OK",
                            String::new(),
                            "<html><head></head><body>Docs</body></html>",
                        )
                    };
                    let response = format!(
                        "HTTP/1.1 {status}\r\n{location}Content-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(response.as_bytes()).await;
                    if method != "HEAD" {
                        let _ = stream.write_all(body.as_bytes()).await;
                    }
                    let _ = stream.shutdown().await;
                });
            }
        });

        let fixture = crate::db::test_helpers::temp_db_arc();
        let db = Arc::clone(&fixture.db);
        let project_id = db
            .upsert_project("Redirect verification", "/tmp/redirect-verification", None)
            .expect("project");
        db.add_environment(project_id, &base_url, "Production", "production", "manual")
            .expect("environment");
        let check_id = crate::core::correlation::resolve_check_id("web_scan", "seo.title");
        crate::db::test_helpers::insert_test_work_item(&db, project_id, &base_url, &check_id)
            .expect("seed authored occurrence");
        let authored_for_db = authored_url.clone();
        db.execute(move |conn| {
            conn.execute(
                "UPDATE work_items
                    SET page_url = ?1, producer_check_id = 'seo.title'
                  WHERE project_id = ?2 AND check_id = ?3",
                rusqlite::params![authored_for_db, project_id, check_id],
            )?;
            Ok::<(), rusqlite::Error>(())
        })
        .expect("database worker")
        .expect("seed occurrence identity");

        run_web_issue_verification(
            None,
            Arc::clone(&db),
            project_id,
            &base_url,
            &authored_url,
            &["seo.title".into()],
        )
        .await
        .expect("run redirected verification");

        let active_rows = db
            .execute(move |conn| {
                conn.prepare(
                    "SELECT signal_id, env_url, page_url, producer_check_id FROM work_items
                      WHERE project_id = ?1 AND source = 'web_scan'
                        AND producer_check_id = 'seo.title'
                        AND resolved_at IS NULL
                      ORDER BY signal_id",
                )?
                .query_map([project_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()
                .map_err(crate::db::DbError::from)
            })
            .expect("database worker")
            .expect("active pages");
        server.abort();

        assert_eq!(
            active_rows,
            vec![(
                format!("web_scan:seo.title:{effective_url}"),
                crate::db::normalize_env_url(Some(&base_url)),
                Some(effective_url),
                Some("seo.title".into()),
            )]
        );
    }
}
