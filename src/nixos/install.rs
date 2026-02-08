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
            let path = tomlcfg::packages_file_path()?;
            let mut pf = tomlcfg::read(std::path::Path::new(&path))?;
            for attr in &pkgs_to_install {
                if !pf.system.packages.contains(attr) {
                    pf.system.packages.push(attr.clone());
                }
            }
            pf.system.packages.sort();
            (toml::to_string_pretty(&pf)?, path)
        }
        ConfigMode::Nix => {
            let oldconfig = config.read_system_config_file()?;
            if let Ok(withvals) =
                nix_editor::read::getwithvalue(&oldconfig, "environment.systemPackages")
                && !withvals.contains(&String::from("pkgs"))
            {
                pkgs_to_install = pkgs_to_install
                    .iter()
                    .map(|x| format!("pkgs.{}", x))
                    .collect();
            }
            let newconfig = nix_editor::write::addtoarr(
                &oldconfig,
                "environment.systemPackages",
                pkgs_to_install,
            )?;
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
