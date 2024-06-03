use crate::{config::configfile, nixos::list::list_systempackages, HELPER_EXEC};
use anyhow::{anyhow, Context, Result};
use log::debug;
use tokio::io::AsyncWriteExt;

use super::AuthMethod;

pub async fn remove(
    pkgs: &[&str],
    db: &rusqlite::Connection,
    auth_method: AuthMethod <'_>,
) -> Result<()> {
    let installed = list_systempackages(db)?
        .into_iter()
        .map(|x| x.attr.to_string())
        .collect::<Vec<_>>();

    // Check if the package is within nixpkgs and if it is installed
    let mut stmt = db.prepare("SELECT pname FROM pkgs WHERE attribute = ?")?;
    let mut pkgs_to_remove = vec![];
    for pkg in pkgs {
        let out: Result<String, _> = stmt.query_row(&[pkg], |row| Ok(row.get(0)?));
        if let Ok(_pname) = out {
            if installed.contains(&pkg.to_string()) {
                pkgs_to_remove.push(pkg.to_string());
            } else {
                debug!("{} is not installed", pkg);
            }
        }
    }

    // Install the packages
    let config = configfile::get_config()?;
    let oldconfig = config.read_system_config_file()?;

    if pkgs_to_remove.is_empty() {
        return Err(anyhow!("No new packages to install"));
    }

    if let Ok(withvals) = nix_editor::read::getwithvalue(&oldconfig, "environment.systemPackages") {
        if !withvals.contains(&String::from("pkgs")) {
            pkgs_to_remove = pkgs_to_remove
                .iter()
                .map(|x| format!("pkgs.{}", x))
                .collect();
        }
    }

    let newconfig =
        nix_editor::write::rmarr(&oldconfig, "environment.systemPackages", pkgs_to_remove)?;

    let mut output = tokio::process::Command::new(match auth_method {
        AuthMethod::Pkexec => "pkexec",
        AuthMethod::Sudo => "sudo",
        AuthMethod::Custom(cmd) => cmd,
    })
    .arg(&*HELPER_EXEC)
    .arg("config")
    .arg("--output")
    .arg(
        &config
            .systemconfig
            .clone()
            .context("Failed to get system config path")?,
    )
    .args(if let Some(generations) = config.get_generation_count() {
        vec!["--generations".to_string(), generations.to_string()]
    } else {
        vec![]
    })
    .arg("--")
    .arg("switch")
    .args(if let Some(flake) = config.flake {
        vec![
            "--flake".to_string(),
            if let Some(host) = config.host {
                format!("{}#{}", flake, host)
            } else {
                flake
            },
        ]
    } else {
        vec![]
    })
    .stdin(std::process::Stdio::piped())
    .spawn()?;
    output
        .stdin
        .as_mut()
        .ok_or("stdin not available")
        .unwrap()
        .write_all(newconfig.as_bytes())
        .await?;
    let output = output.wait().await?;
    debug!("{}", output);
    Ok(())
}
