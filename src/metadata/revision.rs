use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use tokio::process::Command;

use crate::IS_NIXOS;

#[derive(Debug, Deserialize)]
struct NixosVersion {
    #[serde(rename = "nixpkgsRevision")]
    nixpkgs_revision: String,
    #[serde(rename = "nixosVersion")]
    nixos_version: String,
}

#[derive(Debug, Deserialize)]
struct GhResponse {
    sha: String,
}

pub async fn get_revision() -> Result<String> {
    if *IS_NIXOS {
        let output = Command::new("nixos-version").arg("--json").output().await?;
        let output = String::from_utf8(output.stdout)?;
        let version: NixosVersion = serde_json::from_str(&output)?;
        return Ok(version.nixpkgs_revision);
    } else {
        let output = Command::new("nix")
            .arg("registry")
            .arg("list")
            .output()
            .await?;
        let output = String::from_utf8(output.stdout)?;
        let lines = output.split("\n").collect::<Vec<_>>();
        let line = lines
            .iter()
            .find(|x| x.contains("global flake:nixpkgs"))
            .context("No nixpkgs flake found")?;
        let path = line
            .split(" ")
            .collect::<Vec<_>>()
            .get(2)
            .context("Invalid registry entry")?
            .to_string();

        match path {
            x if x.starts_with("github:NixOS/nixpkgs/") => {
                let parts = x.split("/").collect::<Vec<_>>();
                let channel = parts.last().context("Invalid github path")?;
                let output = reqwest::Client::new()
                    .get(&format!(
                        "https://api.github.com/repos/NixOS/nixpkgs/commits/{}",
                        channel
                    ))
                    .header(reqwest::header::USER_AGENT, "libsnow")
                    .send()
                    .await?
                    .json::<GhResponse>()
                    .await?;
                Ok(output.sha)
            }
            _ => Err(anyhow!("Invalid nixpkgs flake path")),
        }
    }
}

pub async fn get_profile_revision() -> Result<String> {
    let output = Command::new("nix")
        .arg("registry")
        .arg("list")
        .output()
        .await?;
    let output = String::from_utf8(output.stdout)?;
    let lines = output.split("\n").collect::<Vec<_>>();
    let line = lines
        .iter()
        .find(|x| x.contains("global flake:nixpkgs") || x.contains("system flake:nixpkgs"))
        .context("No nixpkgs flake found")?;
    let path = line
        .split(" ")
        .collect::<Vec<_>>()
        .get(2)
        .context("Invalid registry entry")?
        .to_string();

    match path {
        x if x.starts_with("github:NixOS/nixpkgs/") => {
            let parts = x.split("/").collect::<Vec<_>>();
            let channel = parts.last().context("Invalid github path")?;
            let output = reqwest::Client::new()
                .get(&format!(
                    "https://api.github.com/repos/NixOS/nixpkgs/commits/{}",
                    channel
                ))
                .header(reqwest::header::USER_AGENT, "libsnow")
                .send()
                .await?
                .json::<GhResponse>()
                .await?;
            Ok(output.sha)
        }
        x if x.starts_with("path:/nix/store/") => Ok(x
            .split("&")
            .find(|x| x.starts_with("rev="))
            .context("No rev found")?
            .split("=")
            .collect::<Vec<_>>()
            .get(1)
            .context("Invalid rev")?
            .to_string()),
        _ => Err(anyhow!("Invalid nixpkgs flake path")),
    }
}

pub async fn get_latest_nixpkgs_revision() -> Result<String> {
    if *IS_NIXOS {
        let output = Command::new("nixos-version").arg("--json").output().await?;
        let output = String::from_utf8(output.stdout)?;

        let version: NixosVersion = serde_json::from_str(&output)?;

        // 24.11.12345678.abcdefg -> 24.11
        let mut release = version
            .nixos_version
            .split(".")
            .take(2)
            .collect::<Vec<_>>()
            .join(".");

        if release == "24.05" {
            release = "unstable".to_string();
        }

        let output = reqwest::Client::new()
            .get(&format!(
                "https://api.github.com/repos/NixOS/nixpkgs/commits/{}",
                format!("nixos-{}", release)
            ))
            .header(reqwest::header::USER_AGENT, "libsnow")
            .send()
            .await?;

        if output.status().is_success() {
            let output = output.json::<GhResponse>().await?;
            Ok(output.sha)
        } else {
            let output = reqwest::Client::new()
                .get(&format!(
                    "https://api.github.com/repos/NixOS/nixpkgs/commits/{}",
                    "nixos-unstable"
                ))
                .header(reqwest::header::USER_AGENT, "libsnow")
                .send()
                .await?;
            let output = output.json::<GhResponse>().await?;
            Ok(output.sha)
        }
    } else {
        let output = reqwest::Client::new()
            .get(&format!(
                "https://api.github.com/repos/NixOS/nixpkgs/commits/{}",
                "nixpkgs-unstable"
            ))
            .header(reqwest::header::USER_AGENT, "libsnow")
            .send()
            .await?;
        let output = output.json::<GhResponse>().await?;
        Ok(output.sha)
    }
}
