pub(crate) mod database;
pub(crate) mod revision;
pub(crate) mod search;

use std::path::{Path, PathBuf};

use anyhow::Result;
use log::info;

use search::{DbSearcher, SearchQuery, get_searcher_from_dir};

pub use search::{SearchResult, build_search_index_in_dir, index_dir_for_db_path};

/// Handle for querying Nix package metadata (SQLite + Tantivy search index).
pub struct Metadata {
    conn: rusqlite::Connection,
    searcher: DbSearcher,
    db_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct PkgInfo {
    pub attribute: String,
    pub pname: String,
    pub version: String,
}

impl Metadata {
    /// Open from a `.db` file, building the search index if needed.
    pub fn open(db_path: &Path) -> Result<Self> {
        let index_dir = index_dir_for_db_path(db_path);
        let conn = rusqlite::Connection::open(db_path)?;

        if !index_dir.exists() {
            info!("Building search index for {} ...", db_path.display());
            build_search_index_in_dir(&conn, &index_dir)?;
            info!("Search index written to {}", index_dir.display());
        }

        let searcher = match get_searcher_from_dir(&index_dir) {
            Ok(s) => s,
            Err(_) => {
                // Index directory exists but is corrupt — rebuild
                build_search_index_in_dir(&conn, &index_dir)?;
                get_searcher_from_dir(&index_dir)?
            }
        };

        Ok(Self {
            conn,
            searcher,
            db_path: db_path.to_path_buf(),
        })
    }

    /// Connect to the current nixpkgs revision database.
    pub async fn connect() -> Result<Self> {
        let rev = revision::get_revision().await?;
        let path = database::fetch_database(&rev, database::DatabaseCacheEntry::Current).await?;
        Self::open(Path::new(&path))
    }

    /// Connect to the latest nixpkgs revision database.
    pub async fn connect_latest() -> Result<Self> {
        let rev = revision::get_latest_nixpkgs_revision().await?;
        let path = database::fetch_database(&rev, database::DatabaseCacheEntry::New).await?;
        Self::open(Path::new(&path))
    }

    pub fn search(
        &self,
        query: &str,
        limit: usize,
        score_threshold: f32,
    ) -> Result<Vec<SearchResult>> {
        search::search(
            &SearchQuery {
                query,
                limit,
                score_threshold,
            },
            &self.searcher,
        )
    }

    /// Look up a package by exact attribute name.
    pub fn get(&self, attribute: &str) -> Result<PkgInfo> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT pname, version FROM pkgs WHERE attribute = ?")?;
        let result = stmt.query_row([attribute], |row| {
            Ok(PkgInfo {
                attribute: attribute.to_string(),
                pname: row.get(0)?,
                version: row.get(1)?,
            })
        })?;
        Ok(result)
    }

    /// Look up packages by pname.
    pub fn get_by_pname(&self, pname: &str) -> Result<Vec<PkgInfo>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT attribute, version FROM pkgs WHERE pname = ?")?;
        let rows = stmt.query_map([pname], |row| {
            Ok(PkgInfo {
                attribute: row.get(0)?,
                pname: pname.to_string(),
                version: row.get(1)?,
            })
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn connection(&self) -> &rusqlite::Connection {
        &self.conn
    }
}
