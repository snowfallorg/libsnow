use crate::{config::configfile, homemanager::list::list, HELPER_EXEC};
use anyhow::{anyhow, Context, Result};
use log::debug;
use tokio::io::AsyncWriteExt;

pub async fn install(pkgs: &[&str], db: &rusqlite::Connection) -> Result<()> {
    let installed = list(db)?
        .into_iter()
        .map(|x| x.attr.to_string())
        .collect::<Vec<_>>();

    // Check if the package is within nixpkgs and if it is installed
    let mut stmt = db.prepare("SELECT pname FROM pkgs WHERE attribute = ?")?;
    let mut pkgs_to_install = vec![];
    for pkg in pkgs {
        let out: Result<String, _> = stmt.query_row(&[pkg], |row| Ok(row.get(0)?));
        if let Ok(pname) = out {
            if installed.contains(&pname) {
                debug!("{} is already installed", pname);
            } else {
                pkgs_to_install.push(pkg.to_string());
            }
        }
    }

    // Install the packages
    let config = configfile::get_config()?;
    let oldconfig = config.read_home_config_file()?;

    if pkgs_to_install.is_empty() {
        return Err(anyhow!("No new packages to install"));
    }

    if let Ok(withvals) = nix_editor::read::getwithvalue(&oldconfig, "home.packages") {
        if !withvals.contains(&String::from("pkgs")) {
            pkgs_to_install = pkgs_to_install
                .iter()
                .map(|x| format!("pkgs.{}", x))
                .collect();
        }
    }

    let newconfig = nix_editor::write::addtoarr(&oldconfig, "home.packages", pkgs_to_install)?;

    let mut output = tokio::process::Command::new(&*HELPER_EXEC)
        .arg("config-home")
        .arg("--output")
        .arg(
            &config
                .homeconfig
                .clone()
                .context("Failed to get home config path")?,
        )
        .args(if let Some(generations) = config.get_generation_count() {
            vec!["--generations".to_string(), generations.to_string()]
        } else {
            vec![]
        })
        .arg("--")
        .arg("switch")
        .args(if let Ok(flakedir) = config.get_flake_dir() {
            vec!["--flake".to_string(), flakedir]
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
