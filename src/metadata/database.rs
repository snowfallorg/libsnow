use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use tokio::fs;

use super::revision::get_revision;

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DatabaseCache {
    current_rev: String,
    new_rev: String,
}

pub enum DatabaseCacheEntry {
    Current,
    New,
}

pub async fn fetch_database(rev: &str, entry: DatabaseCacheEntry) -> Result<String> {
    let cache_file_path = format!("{}/.cache/libsnow/cache.json", std::env::var("HOME")?);
    if !PathBuf::from(&cache_file_path).exists() {
        fs::create_dir_all(PathBuf::from(&cache_file_path).parent().context("Invalid path")?).await?;
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
    // Clean up old databases
    let mut old_dbs = fs::read_dir(format!("{}/.cache/libsnow/", std::env::var("HOME")?)).await?;
    while let Ok(Some(entry)) = old_dbs.next_entry().await {
        let path = entry.path();
        if path.extension().unwrap_or_default() == "db" {
            let path_stem = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if path != PathBuf::from(&outpath)
                && path_stem != cachejson.current_rev
                && path_stem != cachejson.new_rev
            {
                fs::remove_file(path).await?;
            }
        }
    }
    Ok(())
}

pub async fn database_connection() -> Result<rusqlite::Connection> {
    let rev = get_revision().await?;
    let path = fetch_database(&rev, DatabaseCacheEntry::Current).await?;
    Ok(rusqlite::Connection::open(path)?)
}

pub async fn database_connection_offline() -> Result<rusqlite::Connection> {
    let mut dbs = fs::read_dir(format!("{}/.cache/libsnow/", std::env::var("HOME")?)).await?;
    while let Ok(Some(entry)) = dbs.next_entry().await {
        let path = entry.path();
        if path.extension().unwrap_or_default() == "db" {
            return Ok(rusqlite::Connection::open(path)?);
        }
    }
    Err(anyhow!("No database found"))
}
