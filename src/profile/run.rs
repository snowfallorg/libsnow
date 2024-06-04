use anyhow::{anyhow, Result};
use tokio::process::Command;

pub async fn run(pkg: &str, args: &[&str]) -> Result<()> {
    let status = Command::new("nix")
        .arg("--extra-experimental-features")
        .arg("nix-command flakes")
        .arg("run")
        .arg(if pkg.contains('#') || pkg.contains(':') {
            pkg.to_string()
        } else {
            format!("nixpkgs#{}", pkg)
        })
        .arg("--impure")
        .arg("--")
        .args(args)
        .status()
        .await?;

    if !status.success() {
        Err(anyhow!("Failed to install packages"))
    } else {
        Ok(())
    }
}
