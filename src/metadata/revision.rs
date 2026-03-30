use crate::{Error, Result};
use serde::Deserialize;
use tokio::process::Command;

use crate::IS_NIXOS;

#[derive(Debug, Deserialize)]
struct NixosVersionJson {
    #[serde(rename = "nixpkgsRevision")]
    nixpkgs_revision: String,
    #[serde(rename = "nixosVersion")]
    nixos_version: String,
}

#[derive(Debug, Deserialize)]
struct GhResponse {
    sha: String,
}

#[derive(Debug, Clone)]
pub(crate) struct RevisionInfo {
    pub nixpkgs_revision: String,
    pub nixos_release: Option<String>,
}

pub(crate) async fn get_revision() -> Result<RevisionInfo> {
    if *IS_NIXOS {
        let output = Command::new("nixos-version").arg("--json").output().await?;
        let output = String::from_utf8(output.stdout)?;
        let version: NixosVersionJson = serde_json::from_str(&output)?;
        // 24.11.12345678.abcdefg -> 24.11
        let release = version
            .nixos_version
            .split('.')
            .take(2)
            .collect::<Vec<_>>()
            .join(".");
        Ok(RevisionInfo {
            nixpkgs_revision: version.nixpkgs_revision,
            nixos_release: Some(release),
        })
    } else {
        let output = Command::new("nix")
            .arg("registry")
            .arg("list")
            .output()
            .await?;
        let output = String::from_utf8(output.stdout)?;
        let lines = output.split('\n').collect::<Vec<_>>();
        let line = lines
            .iter()
            .find(|x| x.contains("global flake:nixpkgs"))
            .ok_or_else(|| Error::NixRegistry {
                reason: "no nixpkgs flake found".into(),
            })?;
        let path = line
            .split(' ')
            .collect::<Vec<_>>()
            .get(2)
            .ok_or_else(|| Error::NixRegistry {
                reason: "invalid registry entry".into(),
            })?
            .to_string();

        match path {
            x if x.starts_with("github:NixOS/nixpkgs/") => {
                let parts = x.split('/').collect::<Vec<_>>();
                let channel = parts.last().ok_or_else(|| Error::NixRegistry {
                    reason: "invalid github path".into(),
                })?;
                let output = reqwest::Client::new()
                    .get(format!(
                        "https://api.github.com/repos/NixOS/nixpkgs/commits/{}",
                        channel
                    ))
                    .header(reqwest::header::USER_AGENT, "libsnow")
                    .send()
                    .await?
                    .json::<GhResponse>()
                    .await?;
                Ok(RevisionInfo {
                    nixpkgs_revision: output.sha,
                    nixos_release: None,
                })
            }
            _ => Err(Error::NixRegistry {
                reason: "invalid nixpkgs flake path".into(),
            }),
        }
    }
}

pub(crate) async fn get_registry_revision() -> Result<RevisionInfo> {
    let output = Command::new("nix")
        .arg("registry")
        .arg("list")
        .output()
        .await?;
    let output = String::from_utf8(output.stdout)?;

    let priority = |scope: &str| -> u8 {
        match scope {
            "user" => 0,
            "system" => 1,
            "global" => 2,
            _ => 3,
        }
    };

    let mut entries: Vec<(u8, &str)> = output
        .lines()
        .filter(|line| line.contains("flake:nixpkgs "))
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                Some((priority(parts[0]), parts[2]))
            } else {
                None
            }
        })
        .collect();

    entries.sort_by_key(|(p, _)| *p);

    let (_, url) = entries.first().ok_or_else(|| Error::NixRegistry {
        reason: "no nixpkgs flake found in registry".into(),
    })?;

    if url.starts_with("github:NixOS/nixpkgs/") {
        let parts: Vec<&str> = url.split('/').collect();
        let channel = *parts.last().ok_or_else(|| Error::NixRegistry {
            reason: "invalid github path".into(),
        })?;

        if let Some(rev_part) = channel.split('?').find(|p| p.starts_with("rev=")) {
            let rev = rev_part.strip_prefix("rev=").unwrap();
            return Ok(RevisionInfo {
                nixpkgs_revision: rev.to_string(),
                nixos_release: None,
            });
        }

        let channel = channel.split('?').next().unwrap_or(channel);

        let output = reqwest::Client::new()
            .get(format!(
                "https://api.github.com/repos/NixOS/nixpkgs/commits/{}",
                channel
            ))
            .header(reqwest::header::USER_AGENT, "libsnow")
            .send()
            .await?
            .json::<GhResponse>()
            .await?;
        Ok(RevisionInfo {
            nixpkgs_revision: output.sha,
            nixos_release: None,
        })
    } else if url.starts_with("path:") {
        if *IS_NIXOS {
            let output = Command::new("nixos-version").arg("--json").output().await?;
            let output = String::from_utf8(output.stdout)?;
            let version: NixosVersionJson = serde_json::from_str(&output)?;
            let release = version
                .nixos_version
                .split('.')
                .take(2)
                .collect::<Vec<_>>()
                .join(".");
            Ok(RevisionInfo {
                nixpkgs_revision: version.nixpkgs_revision,
                nixos_release: Some(release),
            })
        } else {
            Err(Error::NixRegistry {
                reason: "registry nixpkgs points to a store path but not on NixOS".into(),
            })
        }
    } else {
        Err(Error::NixRegistry {
            reason: format!("unsupported nixpkgs registry entry: {}", url),
        })
    }
}

async fn get_channel_revision(channel: &str) -> Result<String> {
    let url = format!("https://channels.nixos.org/{}/git-revision", channel);
    let resp = reqwest::get(&url).await?.error_for_status()?;
    let rev = resp.text().await?;
    Ok(rev.trim().to_string())
}

pub(crate) async fn get_latest_nixpkgs_revision() -> Result<RevisionInfo> {
    if *IS_NIXOS {
        let output = Command::new("nixos-version").arg("--json").output().await?;
        let output = String::from_utf8(output.stdout)?;

        let version: NixosVersionJson = serde_json::from_str(&output)?;

        // 24.11.12345678.abcdefg -> 24.11
        let release = version
            .nixos_version
            .split('.')
            .take(2)
            .collect::<Vec<_>>()
            .join(".");

        let channel = format!("nixos-{}", release);
        match get_channel_revision(&channel).await {
            Ok(rev) => Ok(RevisionInfo {
                nixpkgs_revision: rev,
                nixos_release: Some(release),
            }),
            Err(_) => {
                let rev = get_channel_revision("nixos-unstable").await?;
                Ok(RevisionInfo {
                    nixpkgs_revision: rev,
                    nixos_release: Some(release),
                })
            }
        }
    } else {
        let rev = get_channel_revision("nixpkgs-unstable").await?;
        Ok(RevisionInfo {
            nixpkgs_revision: rev,
            nixos_release: Some("unstable".to_string()),
        })
    }
}
