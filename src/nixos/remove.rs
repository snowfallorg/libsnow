use super::AuthMethod;
use crate::{
    HELPER_EXEC,
    config::configfile::{self, ConfigMode},
    metadata::Metadata,
    nixos::list::list_systempackages,
    toml as tomlcfg,
};
use anyhow::{Context, Result, anyhow};
use log::debug;
use tokio::io::AsyncWriteExt;

pub async fn remove(pkgs: &[&str], md: &Metadata, auth_method: AuthMethod<'_>) -> Result<()> {
    let config = configfile::get_config()?;

    let installed: Vec<String> = list_systempackages(md)?
        .into_iter()
        .map(|x| x.attr.to_string())
        .collect();

    let mut pkgs_to_remove = vec![];
    for pkg in pkgs {
        if md.get(pkg).is_ok() {
            if installed.contains(&pkg.to_string()) {
                pkgs_to_remove.push(pkg.to_string());
            } else {
                debug!("{} is not installed", pkg);
            }
        }
    }

    if pkgs_to_remove.is_empty() {
        return Err(anyhow!("No packages to remove"));
    }

    let (content, output_path) = match config.mode {
        ConfigMode::Toml => {
            let path = tomlcfg::config_file_path()?;
            let mut pf = tomlcfg::read(std::path::Path::new(&path))?;
            for attr in &pkgs_to_remove {
                pf.system.packages.retain(|p| p != attr);
                let prefix = format!("programs.{}.", attr);
                let keys_to_remove: Vec<String> = pf
                    .system
                    .options
                    .keys()
                    .filter(|k| k.starts_with(&prefix))
                    .cloned()
                    .collect();
                for key in keys_to_remove {
                    pf.system.options.remove(&key);
                }
            }
            (toml::to_string_pretty(&pf)?, path)
        }
        ConfigMode::Nix => {
            let oldconfig = config.read_system_config_file()?;
            if let Ok(withvals) =
                nix_editor::read::getwithvalue(&oldconfig, "environment.systemPackages")
                && !withvals.contains(&String::from("pkgs"))
            {
                pkgs_to_remove = pkgs_to_remove
                    .iter()
                    .map(|x| format!("pkgs.{}", x))
                    .collect();
            }
            let newconfig =
                nix_editor::write::rmarr(&oldconfig, "environment.systemPackages", pkgs_to_remove)?;
            let path = config
                .systemconfig
                .clone()
                .context("Failed to get system config path")?;
            (newconfig, path)
        }
    };

    let mut output = tokio::process::Command::new(match auth_method {
        AuthMethod::Pkexec => "pkexec",
        AuthMethod::Sudo => "sudo",
        AuthMethod::Custom(cmd) => cmd,
    })
    .arg(HELPER_EXEC)
    .arg("config")
    .arg("--output")
    .arg(&output_path)
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
        .write_all(content.as_bytes())
        .await?;
    let output = output.wait().await?;
    debug!("{}", output);
    Ok(())
}
