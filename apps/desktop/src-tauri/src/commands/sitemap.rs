use crate::db::{Database, PageRecord};
use std::sync::Arc;
use tauri::State;

use super::{run_blocking, sanitize_error, validate_url_async};

/// Auto-discover a sitemap for a URL by probing common sitemap paths.
#[tauri::command]
#[tracing::instrument(skip(url))]
pub async fn discover_sitemap(url: String) -> Result<crate::core::sitemap::SitemapResult, String> {
    validate_url_async(&url).await?;
    let parsed = url::Url::parse(&url).map_err(|e| sanitize_error(format!("Invalid URL: {e}")))?;
    let is_strict_local = crate::core::localhost::is_strict_localhost(&parsed);
    let client = crate::http_client::for_url(is_strict_local).clone();
    Ok(crate::core::sitemap::discover_sitemap(&client, &url, is_strict_local).await)
}

/// Fetch a sitemap from a user-specified URL (manual entry).
#[tauri::command]
#[tracing::instrument(skip(sitemap_url))]
pub async fn fetch_sitemap_manual(
    sitemap_url: String,
) -> Result<crate::core::sitemap::SitemapResult, String> {
    validate_url_async(&sitemap_url).await?;
    let parsed =
        url::Url::parse(&sitemap_url).map_err(|e| sanitize_error(format!("Invalid URL: {e}")))?;
    let is_strict_local = crate::core::localhost::is_strict_localhost(&parsed);
    let client = crate::http_client::for_url(is_strict_local).clone();
    Ok(crate::core::sitemap::fetch_sitemap_url(&client, &sitemap_url, is_strict_local).await)
}

/// Save a list of page URLs to the pages table for a site.
#[tauri::command]
#[tracing::instrument(skip(db, urls), fields(site_id, source = %source))]
pub async fn save_site_pages(
    db: State<'_, Arc<Database>>,
    site_id: i64,
    urls: Vec<String>,
    source: String,
) -> Result<usize, String> {
    let db = (*db).clone();
    run_blocking(move || db.save_pages(site_id, &urls, &source))
        .await?
        .map_err(sanitize_error)
}

/// Get all discovered pages for a site (from sitemap or manual entry).
#[tauri::command]
#[tracing::instrument(skip(db), fields(site_id))]
pub async fn get_site_pages(
    db: State<'_, Arc<Database>>,
    site_id: i64,
) -> Result<Vec<PageRecord>, String> {
    let db = (*db).clone();
    run_blocking(move || db.get_pages(site_id))
        .await?
        .map_err(sanitize_error)
}

/// Re-fetch the sitemap for a site: uses stored sitemap URL if available, otherwise auto-discovers.
/// Saves the discovered pages to the DB.
#[tauri::command]
#[tracing::instrument(skip(db, url), fields(site_id))]
pub async fn refresh_sitemap(
    db: State<'_, Arc<Database>>,
    site_id: i64,
    url: String,
) -> Result<crate::core::sitemap::SitemapResult, String> {
    validate_url_async(&url).await?;
    let parsed = url::Url::parse(&url).map_err(|e| sanitize_error(format!("Invalid URL: {e}")))?;
    let is_strict_local = crate::core::localhost::is_strict_localhost(&parsed);
    let client = crate::http_client::for_url(is_strict_local).clone();

    // Check if there's a stored sitemap URL
    let db = (*db).clone();
    let sitemap_url = {
        let db = db.clone();
        run_blocking(move || db.get_sitemap_url(site_id).unwrap_or(None)).await?
    };

    let result = if let Some(ref smap_url) = sitemap_url {
        validate_url_async(smap_url).await?;
        crate::core::sitemap::fetch_sitemap_url(&client, smap_url, is_strict_local).await
    } else {
        crate::core::sitemap::discover_sitemap(&client, &url, is_strict_local).await
    };

    persist_refreshed_sitemap(db, site_id, &result, sitemap_url.is_some()).await?;

    Ok(result)
}

/// Persist a refreshed sitemap: the discovered pages replace the stored ones
/// in one transaction, and the stored sitemap URL follows.
///
/// Every failure is reported. A refresh that cannot be stored must not answer
/// with the fetched result while the site is left holding a wiped or stale
/// page list.
async fn persist_refreshed_sitemap(
    db: Arc<Database>,
    site_id: i64,
    result: &crate::core::sitemap::SitemapResult,
    from_stored_url: bool,
) -> Result<(), String> {
    if result.status != crate::core::sitemap::SitemapStatus::Found || result.urls.is_empty() {
        return Ok(());
    }
    let source = if from_stored_url { "manual" } else { "auto" };
    let urls = result.urls.clone();
    let source_url = result.source_url.clone();
    run_blocking(move || {
        db.replace_pages(site_id, &urls, source)?;
        if let Some(ref src_url) = source_url {
            db.set_sitemap_url(site_id, Some(src_url))?;
        }
        Ok::<(), crate::db::DbError>(())
    })
    .await?
    .map_err(sanitize_error)
}

/// Set or clear the stored sitemap URL for a site.
#[tauri::command]
#[tracing::instrument(skip(db, sitemap_url), fields(site_id))]
pub async fn set_site_sitemap_url(
    db: State<'_, Arc<Database>>,
    site_id: i64,
    sitemap_url: Option<String>,
) -> Result<(), String> {
    if let Some(ref url) = sitemap_url {
        validate_url_async(url).await?;
    }
    let db = (*db).clone();
    run_blocking(move || db.set_sitemap_url(site_id, sitemap_url.as_deref()))
        .await?
        .map_err(sanitize_error)
}

/// Get or create a site record for a URL, returning its site_id.
#[tauri::command]
#[tracing::instrument(skip(db, url))]
pub async fn get_or_create_site_id(
    db: State<'_, Arc<Database>>,
    url: String,
    project_id: Option<i64>,
) -> Result<i64, String> {
    let db = (*db).clone();
    run_blocking(move || match project_id {
        Some(project_id) => db.get_or_create_site_for_project(project_id, &url),
        None => db.get_or_create_site(&url),
    })
    .await?
    .map_err(sanitize_error)
}

#[cfg(test)]
#[path = "sitemap_tests.rs"]
mod tests;
