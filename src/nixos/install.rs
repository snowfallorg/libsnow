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
use toml::Value as TomlValue;

pub async fn install(pkgs: &[&str], md: &Metadata, auth_method: AuthMethod<'_>) -> Result<()> {
    let config = configfile::get_config()?;

    let installed: Vec<String> = list_systempackages(md)?
        .into_iter()
        .map(|x| x.attr.to_string())
        .collect();

    let mut pkgs_to_install = vec![];
    for pkg in pkgs {
        if let Ok(info) = md.get(pkg) {
            if installed.contains(&info.pname) {
                debug!("{} is already installed", info.pname);
            } else {
                pkgs_to_install.push(pkg.to_string());
            }
        }
    }

    if pkgs_to_install.is_empty() {
        return Err(anyhow!("No new packages to install"));
    }

    let (content, output_path) = match config.mode {
        ConfigMode::Toml => {
            let path = tomlcfg::system_config_file_path()?;
            let mut pf = tomlcfg::read_system(std::path::Path::new(&path))?;
            for attr in &pkgs_to_install {
                if md.has_program_option(attr) {
                    let key = format!("programs.{}.enable", attr);
                    if pf.options.get(&key) != Some(&TomlValue::Boolean(true)) {
                        pf.options.insert(key, TomlValue::Boolean(true));
                    }
                } else if !pf.packages.contains(attr) {
                    pf.packages.push(attr.clone());
                }
            }
            pf.packages.sort();
            (toml::to_string_pretty(&pf)?, path)
        }
        ConfigMode::Nix => {
            let mut current = config.read_system_config_file()?;
            let mut arr_pkgs = vec![];
            for attr in &pkgs_to_install {
                if md.has_program_option(attr) {
                    let key = format!("programs.{}.enable", attr);
                    current = nix_editor::write::write(&current, &key, "true")
                        .map_err(|e| anyhow!("{}", e))?;
                } else {
                    arr_pkgs.push(attr.clone());
                }
            }
            if !arr_pkgs.is_empty() {
                if let Ok(withvals) =
                    nix_editor::read::getwithvalue(&current, "environment.systemPackages")
                    && !withvals.contains(&String::from("pkgs"))
                {
                    arr_pkgs = arr_pkgs.iter().map(|x| format!("pkgs.{}", x)).collect();
                }
                current =
                    nix_editor::write::addtoarr(&current, "environment.systemPackages", arr_pkgs)
                        .map_err(|e| anyhow!("{}", e))?;
            }
            let path = config
                .system_config_file
                .clone()
                .context("Failed to get system config path")?;
            (current, path)
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
        .write_all(content.as_bytes())
        .await?;
    let output = output.wait().await?;
    debug!("{}", output);
    Ok(())
}
