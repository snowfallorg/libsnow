use crate::{
    HELPER_EXEC,
    config::configfile::{self, ConfigMode},
    homemanager::list::list,
    metadata::Metadata,
    nixos::AuthMethod,
    toml as tomlcfg,
};
use anyhow::{Context, Result, anyhow};
use log::debug;
use tokio::io::AsyncWriteExt;

pub async fn remove(pkgs: &[&str], md: &Metadata, auth_method: AuthMethod<'_>) -> Result<()> {
    let mut child = remove_spawn(pkgs, md, auth_method).await?;
    let status = child.wait().await?;
    debug!("{}", status);
    if !status.success() {
        return Err(anyhow!("Failed to rebuild"));
    }
    Ok(())
}

pub async fn remove_spawn(
    pkgs: &[&str],
    md: &Metadata,
    auth_method: AuthMethod<'_>,
) -> Result<tokio::process::Child> {
    let config = configfile::get_config()?;

    let installed: Vec<String> = list(md)
        .unwrap_or_default()
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
            let user = tomlcfg::current_user()?;
            let path = tomlcfg::home_config_file_path()?;
            let mut pf = tomlcfg::read_home(std::path::Path::new(&path))?;
            if let Some(section) = pf.users.get_mut(&user) {
                for attr in &pkgs_to_remove {
                    section.packages.retain(|p| p != attr);
                    let prefix = format!("programs.{}.", attr);
                    let keys_to_remove: Vec<String> = section
                        .options
                        .keys()
                        .filter(|k| k.starts_with(&prefix))
                        .cloned()
                        .collect();
                    for key in keys_to_remove {
                        section.options.remove(&key);
                    }
                }
            }
            (toml::to_string_pretty(&pf)?, path)
        }
        ConfigMode::Nix => {
            let mut current = config.read_home_config_file()?;
            let mut arr_pkgs = vec![];
            for attr in &pkgs_to_remove {
                let key = format!("programs.{}.enable", attr);
                if nix_editor::read::readvalue(&current, &key).is_ok() {
                    current =
                        nix_editor::write::deref(&current, &key).map_err(|e| anyhow!("{}", e))?;
                } else {
                    arr_pkgs.push(attr.clone());
                }
            }
            if !arr_pkgs.is_empty() {
                if let Ok(withvals) = nix_editor::read::getwithvalue(&current, "home.packages")
                    && !withvals.contains(&String::from("pkgs"))
                {
                    arr_pkgs = arr_pkgs.iter().map(|x| format!("pkgs.{}", x)).collect();
                }
                current = nix_editor::write::rmarr(&current, "home.packages", arr_pkgs)
                    .map_err(|e| anyhow!("{}", e))?;
            }
            let path = config
                .home_config_file
                .clone()
                .context("Failed to get home config path")?;
            (current, path)
        }
    };

    let mut child = tokio::process::Command::new(if config.system_for_home_manager {
        match auth_method {
            AuthMethod::Pkexec => "pkexec",
            AuthMethod::Sudo => "sudo",
            AuthMethod::Custom(cmd) => cmd,
        }
    } else {
        HELPER_EXEC
    })
    .args(if config.system_for_home_manager {
        vec![HELPER_EXEC, "config"]
    } else {
        vec!["config-home"]
    })
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

    child
        .stdin
        .as_mut()
        .ok_or("stdin not available")
        .unwrap()
        .write_all(content.as_bytes())
        .await?;
    child.stdin.take();

    Ok(child)
}
