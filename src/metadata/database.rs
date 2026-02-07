use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use tokio::fs;

use super::search::index_dir_for_db_path;

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DatabaseCache {
    current_rev: String,
    new_rev: String,
}

pub(crate) enum DatabaseCacheEntry {
    Current,
    New,
}

pub(crate) async fn fetch_database(rev: &str, entry: DatabaseCacheEntry) -> Result<String> {
    let cache_file_path = format!("{}/.cache/libsnow/cache.json", std::env::var("HOME")?);
    if !PathBuf::from(&cache_file_path).exists() {
        fs::create_dir_all(
            PathBuf::from(&cache_file_path)
                .parent()
                .context("Invalid path")?,
        )
        .await?;
        fs::write(&cache_file_path, r#"{"current_rev": "", "new_rev": ""}"#).await?;
    }
    let cache_content = fs::read_to_string(&cache_file_path).await?;
    let mut cachejson: DatabaseCache = serde_json::from_str(&cache_content)?;
    match entry {
        DatabaseCacheEntry::Current if rev != cachejson.current_rev => {
            cachejson.current_rev = rev.to_string();
        }
        DatabaseCacheEntry::New if rev != cachejson.new_rev => {
            cachejson.new_rev = rev.to_string();
        }
        _ => {}
    }
    fs::write(&cache_file_path, serde_json::to_string(&cachejson)?).await?;

    let outpath = format!("{}/.cache/libsnow/{}.db", std::env::var("HOME")?, rev);

    if PathBuf::from(&outpath).exists() {
        cleanup(&outpath, &cachejson).await?;
        return Ok(outpath);
    }

    let client = reqwest::Client::builder().brotli(true).build()?;
    let output = client
        .get(format!("https://api.snowflakeos.org/libsnow/{}", rev))
        .send()
        .await?;

    let status = output.status();
    if !status.is_success() {
        return Err(anyhow!("Failed to fetch database: {}", status));
    }

    fs::create_dir_all(PathBuf::from(&outpath).parent().context("Invalid path")?).await?;
    fs::write(&outpath, output.bytes().await?).await?;

    cleanup(&outpath, &cachejson).await?;

    Ok(outpath)
}

async fn cleanup(outpath: &str, cachejson: &DatabaseCache) -> Result<()> {
    // Clean up old databases and their search indexes
    let cache_dir = format!("{}/.cache/libsnow/", std::env::var("HOME")?);
    let mut entries = fs::read_dir(&cache_dir).await?;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let ext = path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        if ext != "db" && ext != "index" {
            continue;
        }

        let stem = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Keep entries belonging to current or new revision
        if stem == cachejson.current_rev || stem == cachejson.new_rev {
            continue;
        }

        // Keep the outpath itself and its sibling index
        let outpath_pb = PathBuf::from(outpath);
        if path == outpath_pb || path == index_dir_for_db_path(&outpath_pb) {
            continue;
        }

        if path.is_dir() {
            let _ = fs::remove_dir_all(&path).await;
        } else {
            let _ = fs::remove_file(&path).await;
        }
    }
    Ok(())
}
