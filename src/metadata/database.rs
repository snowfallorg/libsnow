use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use tokio::fs;

use super::revision::get_revision;

pub async fn fetch_database(rev: &str, clean: bool) -> Result<String> {
    let outpath = format!("{}/.cache/libsnow/{}.db", std::env::var("HOME")?, rev);

    if PathBuf::from(&outpath).exists() {
        if clean {
            cleanup(&outpath).await?;
        }
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

    if clean {
        cleanup(&outpath).await?;
    }

    Ok(outpath)
}

// TODO: Fix cleanup tracking
async fn cleanup(outpath: &str) -> Result<()> {
    // Clean up old databases
    let mut old_dbs = fs::read_dir(format!("{}/.cache/libsnow/", std::env::var("HOME")?)).await?;
    while let Ok(Some(entry)) = old_dbs.next_entry().await {
        let path = entry.path();
        if path.extension().unwrap_or_default() == "db" {
            if path != PathBuf::from(&outpath) {
                fs::remove_file(path).await?;
            }
        }
    }
    Ok(())
}

pub async fn database_connection() -> Result<rusqlite::Connection> {
    let rev = get_revision().await?;
    let path = fetch_database(&rev, true).await?;
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