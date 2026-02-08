use std::collections::HashMap;

use anyhow::{Context, Result};
use clap::Parser;
use libsnow::metadata::{build_search_index_in_dir, index_dir_for_db_path};
use log::info;
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::Value;

#[derive(Parser, Debug)]
#[command(about = "Generate the libsnow SQLite package database from nixos releases")]
struct Args {
    /// Channel path, e.g. "nixos/unstable/nixos-24.11pre123456.abcdef0"
    /// or just a channel prefix like "nixpkgs" / "nixos/unstable".
    /// When a full release name is given it is used directly.
    /// When only a channel prefix is given the latest release is resolved automatically.
    #[arg(short, long)]
    channel: String,

    /// Specific release name inside the channel (e.g. "nixos-24.11pre123456.abcdef0").
    /// If omitted the latest release is fetched from the S3 bucket listing.
    #[arg(short, long)]
    release: Option<String>,

    /// Output directory for the generated .db file (default: current directory)
    #[arg(short, long, default_value = ".")]
    output: String,

    /// Verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Also generate a persisted Tantivy search index next to the .db file
    #[arg(long)]
    with_index: bool,
}

#[derive(Debug, Deserialize, Clone)]
struct MetaData {
    pub description: Option<String>,
    #[serde(rename = "longDescription")]
    pub long_description: Option<String>,
    pub branch: Option<String>,
    pub homepage: Option<Value>,
    #[serde(rename = "downloadPage")]
    pub download_page: Option<Value>,
    pub changelog: Option<Value>,
    pub license: Option<Value>,
    pub maintainers: Option<Value>,
    #[serde(rename = "mainProgram")]
    pub main_program: Option<String>,
    pub platforms: Option<Value>,
    #[serde(rename = "badPlatforms")]
    pub bad_platforms: Option<Value>,
    pub broken: Option<bool>,
    pub unfree: Option<bool>,
    pub insecure: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct Package {
    meta: Option<MetaData>,
    pname: String,
    version: String,
}

#[derive(Debug, Deserialize)]
struct PkgJson {
    packages: HashMap<String, Package>,
}

#[derive(Debug, Deserialize)]
struct ListBucketResult {
    #[serde(rename = "Contents", default)]
    contents: Vec<S3Content>,
    #[serde(rename = "IsTruncated")]
    is_truncated: bool,
}

#[derive(Debug, Deserialize, Clone)]
struct S3Content {
    #[serde(rename = "Key")]
    pub key: String,
    #[serde(rename = "LastModified")]
    pub last_modified: String,
}

async fn resolve_latest_release(channel: &str) -> Result<String> {
    let url = format!(
        "https://nix-releases.s3.amazonaws.com/?delimiter=/&prefix={}/",
        channel
    );

    let mut all_objects: Vec<S3Content> = Vec::new();
    let mut marker = String::new();
    let mut truncated = true;

    while truncated {
        let resp = reqwest::get(format!("{}&marker={}", url, marker))
            .await?
            .text()
            .await?;
        let result: ListBucketResult = quick_xml::de::from_str(&resp)?;

        for content in &result.contents {
            all_objects.push(content.clone());
        }

        truncated = result.is_truncated;
        if let Some(last) = result.contents.last() {
            marker = last.key.clone();
        }
    }

    all_objects.sort_by(|a, b| a.last_modified.cmp(&b.last_modified));

    let latest = all_objects
        .last()
        .context("No releases found for channel")?;

    let release_name = latest
        .key
        .trim_matches('/')
        .split('/')
        .next_back()
        .context("Invalid key format")?
        .to_string();

    Ok(release_name)
}

async fn fetch_packages(channel: &str, release: &str) -> Result<HashMap<String, Package>> {
    let url = format!(
        "https://releases.nixos.org/{}/{}/packages.json.br",
        channel, release
    );

    info!("Fetching {}", url);

    let client = reqwest::Client::builder().brotli(true).build()?;
    let bytes = client.get(&url).send().await?.bytes().await?;
    let pkg_json: PkgJson = serde_json::from_slice(&bytes)?;

    Ok(pkg_json.packages)
}

async fn fetch_program_options(
    channel: &str,
    release: &str,
) -> Result<HashMap<String, HashMap<String, Value>>> {
    let url = format!(
        "https://releases.nixos.org/{}/{}/options.json.br",
        channel, release
    );

    info!("Fetching {}", url);

    let client = reqwest::Client::builder().brotli(true).build()?;
    let resp = client.get(&url).send().await?;

    if !resp.status().is_success() {
        info!("Options file not available ({}), skipping", resp.status());
        return Ok(HashMap::new());
    }

    let bytes = resp.bytes().await?;
    let all_options: HashMap<String, Value> = serde_json::from_slice(&bytes)?;
    let programs = extract_program_options(all_options);

    info!("Found {} programs with NixOS options", programs.len());
    Ok(programs)
}

fn extract_program_options(
    all_options: HashMap<String, Value>,
) -> HashMap<String, HashMap<String, Value>> {
    let mut programs: HashMap<String, HashMap<String, Value>> = HashMap::new();
    for (key, value) in all_options {
        if let Some(rest) = key.strip_prefix("programs.")
            && let Some(prog_name) = rest.split('.').next()
        {
            programs
                .entry(prog_name.to_string())
                .or_default()
                .insert(key, value);
        }
    }
    programs.retain(|name, opts| opts.contains_key(&format!("programs.{}.enable", name)));
    programs
}

fn fetch_hm_program_options(channel: &str) -> Result<HashMap<String, HashMap<String, Value>>> {
    let hm_branch = if channel.contains("unstable") || channel.starts_with("nixpkgs") {
        "master".to_string()
    } else {
        // Extract version like "25.05" from "nixos/25.05"
        let version = channel.rsplit('/').next().unwrap_or("master");
        format!("release-{}", version)
    };

    info!(
        "Building home-manager docs-json from branch '{}'",
        hm_branch
    );

    let output = std::process::Command::new("nix")
        .args([
            "build",
            &format!("github:nix-community/home-manager/{}#docs-json", hm_branch),
            "--no-link",
            "--print-out-paths",
        ])
        .output()
        .context("Failed to run nix build for home-manager docs-json")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        info!("home-manager docs-json build failed, skipping: {}", stderr);
        return Ok(HashMap::new());
    }

    let store_path = String::from_utf8(output.stdout)?.trim().to_string();
    let json_path = format!("{}/share/doc/home-manager/options.json", store_path);

    info!("Reading HM options from {}", json_path);
    let content = std::fs::read_to_string(&json_path)
        .with_context(|| format!("Failed to read {}", json_path))?;
    let all_options: HashMap<String, Value> = serde_json::from_str(&content)?;

    let programs = extract_program_options(all_options);
    info!(
        "Found {} programs with home-manager options",
        programs.len()
    );
    Ok(programs)
}

async fn fetch_git_revision(channel: &str, release: &str) -> Result<String> {
    let url = format!(
        "https://releases.nixos.org/{}/{}/git-revision",
        channel, release
    );
    let rev = reqwest::get(&url).await?.text().await?;
    Ok(rev.trim().to_string())
}

fn create_database(
    packages: &HashMap<String, Package>,
    program_options: &HashMap<String, HashMap<String, Value>>,
    hm_program_options: &HashMap<String, HashMap<String, Value>>,
    db_path: &str,
) -> Result<()> {
    // Remove old db if present
    let _ = std::fs::remove_file(db_path);

    let conn = Connection::open(db_path)?;

    conn.execute_batch("PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF;")?;

    conn.execute(
        r#"CREATE TABLE pkgs (
            "attribute" TEXT NOT NULL UNIQUE,
            "pname" TEXT,
            "version" TEXT,
            PRIMARY KEY("attribute")
        )"#,
        [],
    )?;

    conn.execute(
        r#"CREATE TABLE meta (
            "attribute" TEXT NOT NULL UNIQUE,
            "description" TEXT,
            "long_description" TEXT,
            "branch" TEXT,
            "homepage" JSON,
            "download_page" JSON,
            "changelog" JSON,
            "license" JSON,
            "maintainers" JSON,
            "main_program" TEXT,
            "platforms" JSON,
            "bad_platforms" JSON,
            "broken" INTEGER,
            "unfree" INTEGER,
            "insecure" INTEGER,
            FOREIGN KEY("attribute") REFERENCES "pkgs" ("attribute"),
            PRIMARY KEY("attribute")
        )"#,
        [],
    )?;

    conn.execute(
        r#"CREATE TABLE program_options (
            "attribute" TEXT NOT NULL UNIQUE,
            "options" JSON NOT NULL,
            FOREIGN KEY("attribute") REFERENCES "pkgs" ("attribute"),
            PRIMARY KEY("attribute")
        )"#,
        [],
    )?;

    conn.execute(
        r#"CREATE TABLE hm_program_options (
            "attribute" TEXT NOT NULL UNIQUE,
            "options" JSON NOT NULL,
            FOREIGN KEY("attribute") REFERENCES "pkgs" ("attribute"),
            PRIMARY KEY("attribute")
        )"#,
        [],
    )?;

    conn.execute(r#"CREATE INDEX "idx_pkgs" ON "pkgs" ("attribute")"#, [])?;
    conn.execute(r#"CREATE INDEX "idx_meta" ON "meta" ("attribute")"#, [])?;
    conn.execute(
        r#"CREATE INDEX "idx_program_options" ON "program_options" ("attribute")"#,
        [],
    )?;
    conn.execute(
        r#"CREATE INDEX "idx_hm_program_options" ON "hm_program_options" ("attribute")"#,
        [],
    )?;

    // Insert in a single transaction for speed
    conn.execute_batch("BEGIN")?;

    {
        let mut pkg_stmt = conn.prepare(
            "INSERT OR IGNORE INTO pkgs (attribute, pname, version) VALUES (?1, ?2, ?3)",
        )?;
        let mut meta_stmt = conn.prepare(
            "INSERT OR IGNORE INTO meta (attribute, description, long_description, branch, homepage, download_page, changelog, license, maintainers, main_program, platforms, bad_platforms, broken, unfree, insecure) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        )?;

        for (attr, pkg) in packages {
            pkg_stmt.execute(rusqlite::params![attr, pkg.pname, pkg.version])?;

            let meta = match &pkg.meta {
                Some(m) => m,
                None => continue,
            };

            let bool_to_int =
                |b: Option<bool>| -> i32 { b.map(|v| if v { 1 } else { 0 }).unwrap_or(0) };

            meta_stmt.execute(rusqlite::params![
                attr,
                meta.description.as_deref().unwrap_or_default(),
                meta.long_description.as_deref().unwrap_or_default(),
                meta.branch.as_deref().unwrap_or_default(),
                meta.homepage
                    .as_ref()
                    .map(|x| x.to_string())
                    .unwrap_or_default(),
                meta.download_page
                    .as_ref()
                    .map(|x| x.to_string())
                    .unwrap_or_default(),
                meta.changelog
                    .as_ref()
                    .map(|x| x.to_string())
                    .unwrap_or_default(),
                meta.license
                    .as_ref()
                    .map(|x| x.to_string())
                    .unwrap_or_default(),
                meta.maintainers
                    .as_ref()
                    .map(|x| x.to_string())
                    .unwrap_or_default(),
                meta.main_program.as_deref().unwrap_or_default(),
                meta.platforms
                    .as_ref()
                    .map(|x| x.to_string())
                    .unwrap_or_default(),
                meta.bad_platforms
                    .as_ref()
                    .map(|x| x.to_string())
                    .unwrap_or_default(),
                bool_to_int(meta.broken),
                bool_to_int(meta.unfree),
                bool_to_int(meta.insecure),
            ])?;
        }
    }

    {
        let mut opt_stmt = conn.prepare(
            "INSERT OR IGNORE INTO program_options (attribute, options) VALUES (?1, ?2)",
        )?;
        for (prog, opts) in program_options {
            let json = serde_json::to_string(opts)?;
            opt_stmt.execute(rusqlite::params![prog, json])?;
        }

        let mut hm_stmt = conn.prepare(
            "INSERT OR IGNORE INTO hm_program_options (attribute, options) VALUES (?1, ?2)",
        )?;
        for (prog, opts) in hm_program_options {
            let json = serde_json::to_string(opts)?;
            hm_stmt.execute(rusqlite::params![prog, json])?;
        }
    }

    conn.execute_batch("COMMIT")?;

    info!("Database written to {}", db_path);
    Ok(())
}

fn create_search_index(db_path: &str) -> Result<()> {
    let conn = Connection::open(db_path)?;
    let index_dir = index_dir_for_db_path(std::path::Path::new(db_path));
    build_search_index_in_dir(&conn, &index_dir)?;
    info!("Search index written to {}", index_dir.display());
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if args.verbose {
        let mut logger = pretty_env_logger::formatted_timed_builder();
        logger.parse_filters("generate_db=debug,info");
        logger.try_init()?;
    } else {
        pretty_env_logger::try_init()?;
    }

    let channel = &args.channel;

    // Resolve the release name
    let release = match args.release {
        Some(r) => r,
        None => {
            info!(
                "No release specified, resolving latest for channel '{}'",
                channel
            );
            let r = resolve_latest_release(channel).await?;
            info!("Resolved latest release: {}", r);
            r
        }
    };

    // Fetch git revision (used as the db filename, matching the original generator)
    let git_rev = fetch_git_revision(channel, &release).await?;
    info!("Git revision: {}", git_rev);

    // Fetch package metadata and program options
    info!("Fetching package metadata for {}/{} ...", channel, release);
    let packages = fetch_packages(channel, &release).await?;
    info!("Got {} packages", packages.len());

    let program_options = fetch_program_options(channel, &release).await?;
    let hm_program_options = fetch_hm_program_options(channel)?;

    // Build the database
    let db_path = format!("{}/{}.db", args.output, git_rev);
    create_database(&packages, &program_options, &hm_program_options, &db_path)?;
    if args.with_index {
        info!("Building search index ...");
        create_search_index(&db_path)?;
    }

    eprintln!("{}", db_path);

    Ok(())
}
