use anyhow::{anyhow, Result};
use tokio::process::Command;

pub async fn install(package: &str) -> Result<()> {
    let status = Command::new("nix-env")
        .arg("-iA")
        .arg(&format!("nixos.{}", package))
        .status()
        .await?;

    if status.success() {
        Err(anyhow!("Failed to install {}", package))
    } else {
        Ok(())
    }
}

pub async fn available_version(pkg: &str) -> Result<String> {
    // Instead get version from revision
    // nix flake metadata nixpkgs
    let output = Command::new("nix-instantiate")
        .arg("--eval")
        .arg("-E")
        .arg(&format!("with import <nixpkgs> {{}}; pkgs.{}.version", pkg))
        .output()
        .await?;
    let version = String::from_utf8(output.stdout)?;
    Ok(version.trim_matches('\"').to_string())
}
