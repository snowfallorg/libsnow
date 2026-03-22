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
    nixpkgs_revision: Option<String>,
    nixos_release: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AliasKind {
    Rename {
        replacement: String,
        message: Option<String>,
    },
    Removed {
        message: String,
    },
}

#[derive(Debug, Clone)]
pub struct AliasInfo {
    pub attribute: String,
    pub kind: AliasKind,
}

#[derive(Debug, Clone)]
pub struct PkgInfo {
    pub attribute: String,
    pub pname: String,
    pub version: String,
    pub description: Option<String>,
    pub broken: bool,
    pub insecure: bool,
    pub unfree: bool,
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
            nixpkgs_revision: None,
            nixos_release: None,
        })
    }

    /// Connect to the current nixpkgs revision database.
    pub async fn connect() -> Result<Self> {
        let info = revision::get_revision().await?;
        let path = database::fetch_database(
            &info.nixpkgs_revision,
            database::DatabaseCacheEntry::Current,
        )
        .await?;
        let mut md = Self::open(Path::new(&path))?;
        md.nixpkgs_revision = Some(info.nixpkgs_revision);
        md.nixos_release = info.nixos_release;
        Ok(md)
    }

    /// Connect to the nixpkgs revision from the user's nix registry.
    pub async fn connect_registry() -> Result<Self> {
        let info = revision::get_registry_revision().await?;
        let path = database::fetch_database(
            &info.nixpkgs_revision,
            database::DatabaseCacheEntry::Current,
        )
        .await?;
        let mut md = Self::open(Path::new(&path))?;
        md.nixpkgs_revision = Some(info.nixpkgs_revision);
        md.nixos_release = info.nixos_release;
        Ok(md)
    }

    /// Connect to the latest nixpkgs revision database.
    pub async fn connect_latest() -> Result<Self> {
        let info = revision::get_latest_nixpkgs_revision().await?;
        let path =
            database::fetch_database(&info.nixpkgs_revision, database::DatabaseCacheEntry::New)
                .await?;
        let mut md = Self::open(Path::new(&path))?;
        md.nixpkgs_revision = Some(info.nixpkgs_revision);
        md.nixos_release = info.nixos_release;
        Ok(md)
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
        let mut stmt = self.conn.prepare_cached(
            "SELECT p.pname, p.version, m.description, m.broken, m.insecure, m.unfree \
             FROM pkgs p LEFT JOIN meta m ON p.attribute = m.attribute \
             WHERE p.attribute = ?",
        )?;
        let result = stmt.query_row([attribute], |row| {
            Ok(PkgInfo {
                attribute: attribute.to_string(),
                pname: row.get(0)?,
                version: row.get(1)?,
                description: row.get(2)?,
                broken: row.get::<_, Option<i64>>(3)?.unwrap_or(0) != 0,
                insecure: row.get::<_, Option<i64>>(4)?.unwrap_or(0) != 0,
                unfree: row.get::<_, Option<i64>>(5)?.unwrap_or(0) != 0,
            })
        })?;
        Ok(result)
    }

    /// Look up packages by pname.
    pub fn get_by_pname(&self, pname: &str) -> Result<Vec<PkgInfo>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT p.attribute, p.version, m.description, m.broken, m.insecure, m.unfree \
             FROM pkgs p LEFT JOIN meta m ON p.attribute = m.attribute \
             WHERE p.pname = ?",
        )?;
        let rows = stmt.query_map([pname], |row| {
            Ok(PkgInfo {
                attribute: row.get(0)?,
                pname: pname.to_string(),
                version: row.get(1)?,
                description: row.get(2)?,
                broken: row.get::<_, Option<i64>>(3)?.unwrap_or(0) != 0,
                insecure: row.get::<_, Option<i64>>(4)?.unwrap_or(0) != 0,
                unfree: row.get::<_, Option<i64>>(5)?.unwrap_or(0) != 0,
            })
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Return all attribute names that have a NixOS `programs.<name>.enable` option.
    pub fn all_program_option_attrs(&self) -> Vec<String> {
        self.conn
            .prepare_cached("SELECT attribute FROM program_options")
            .and_then(|mut stmt| {
                let rows = stmt.query_map([], |row| row.get(0))?;
                rows.collect()
            })
            .unwrap_or_default()
    }

    /// Return all attribute names that have a home-manager `programs.<name>.enable` option.
    pub fn all_hm_program_option_attrs(&self) -> Vec<String> {
        self.conn
            .prepare_cached("SELECT attribute FROM hm_program_options")
            .and_then(|mut stmt| {
                let rows = stmt.query_map([], |row| row.get(0))?;
                rows.collect()
            })
            .unwrap_or_default()
    }

    /// Check whether a package has a corresponding `programs.<name>.enable`
    /// NixOS option in the database.
    pub fn has_program_option(&self, attribute: &str) -> bool {
        self.conn
            .prepare_cached("SELECT 1 FROM program_options WHERE attribute = ?")
            .and_then(|mut stmt| stmt.exists([attribute]))
            .unwrap_or(false)
    }

    /// Check whether a package has a corresponding `programs.<name>.enable`
    /// home-manager option in the database.
    pub fn has_hm_program_option(&self, attribute: &str) -> bool {
        self.conn
            .prepare_cached("SELECT 1 FROM hm_program_options WHERE attribute = ?")
            .and_then(|mut stmt| stmt.exists([attribute]))
            .unwrap_or(false)
    }

    /// Look up an alias by the deprecated attribute name.
    pub fn get_alias(&self, attribute: &str) -> Option<AliasInfo> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT type, replacement, message FROM aliases WHERE alias = ?")
            .ok()?;
        stmt.query_row([attribute], |row| {
            let type_str: String = row.get(0)?;
            let replacement: Option<String> = row.get(1)?;
            let message: Option<String> = row.get(2)?;
            let kind = if type_str == "rename" {
                AliasKind::Rename {
                    replacement: replacement.unwrap_or_default(),
                    message,
                }
            } else {
                AliasKind::Removed {
                    message: message.unwrap_or_default(),
                }
            };
            Ok(AliasInfo {
                attribute: attribute.to_string(),
                kind,
            })
        })
        .ok()
    }

    pub fn nixpkgs_revision(&self) -> Option<&str> {
        self.nixpkgs_revision.as_deref()
    }

    pub fn nixos_release(&self) -> Option<&str> {
        self.nixos_release.as_deref()
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn connection(&self) -> &rusqlite::Connection {
        &self.conn
    }
}
