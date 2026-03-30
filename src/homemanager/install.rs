use crate::{
    Error, HELPER_EXEC, Result,
    config::configfile::{self, ConfigMode, LibSnowConfig},
    dbus,
    homemanager::list::list,
    metadata::Metadata,
    nixos::AuthMethod,
    toml as tomlcfg,
};
use tokio::io::AsyncWriteExt;
use toml::Value as TomlValue;
use tracing::debug;

pub async fn install(pkgs: &[&str], md: &Metadata, auth_method: AuthMethod<'_>) -> Result<()> {
    match auth_method {
        AuthMethod::Dbus => install_dbus(pkgs, md).await,
        _ => {
            let mut child = install_spawn(pkgs, md, auth_method).await?;
            let status = child.wait().await?;
            debug!("{}", status);
            if !status.success() {
                return Err(Error::SubprocessFailed {
                    reason: "failed to rebuild".into(),
                });
            }
            Ok(())
        }
    }
}

async fn install_dbus(pkgs: &[&str], md: &Metadata) -> Result<()> {
    let config = configfile::get_config()?;
    let (content, _output_path, _arguments) = prepare_install(pkgs, md, &config)?;

    if config.system_for_home_manager {
        dbus::config(&content, "switch").await
    } else {
        dbus::config_home(&content, "switch").await
    }
}

fn prepare_install(
    pkgs: &[&str],
    md: &Metadata,
    config: &LibSnowConfig,
) -> Result<(String, String, Vec<String>)> {
    let installed: Vec<String> = list(md)
        .unwrap_or_default()
        .into_iter()
        .map(|x| x.attr.to_string())
        .collect();

    let mut pkgs_to_install = vec![];
    for pkg in pkgs {
        if md.get(pkg).is_ok() {
            if installed.contains(&pkg.to_string()) {
                debug!("{} is already installed", pkg);
            } else {
                pkgs_to_install.push(pkg.to_string());
            }
        }
    }

    if pkgs_to_install.is_empty() {
        return Err(Error::NothingToDo {
            reason: "no new packages to install".into(),
        });
    }

    let (content, output_path) = match config.mode {
        ConfigMode::Toml => {
            let user = tomlcfg::current_user()?;
            let path = tomlcfg::home_config_file_path()?;
            let mut pf = tomlcfg::read_home(std::path::Path::new(&path))?;
            let section = pf.users.entry(user).or_default();
            for attr in &pkgs_to_install {
                if md.has_hm_program_option(attr) {
                    let key = format!("programs.{}.enable", attr);
                    if section.options.get(&key) != Some(&TomlValue::Boolean(true)) {
                        section.options.insert(key, TomlValue::Boolean(true));
                    }
                } else if !section.packages.contains(attr) {
                    section.packages.push(attr.clone());
                }
            }
            section.packages.sort();
            (toml::to_string_pretty(&pf)?, path)
        }
        ConfigMode::Nix => {
            let mut current = config.read_home_config_file()?;
            let mut arr_pkgs = vec![];
            for attr in &pkgs_to_install {
                if md.has_hm_program_option(attr) {
                    let key = format!("programs.{}.enable", attr);
                    current = nix_editor::write::write(&current, &key, "true").map_err(|e| {
                        Error::NixEditor {
                            reason: e.to_string(),
                        }
                    })?;
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
                current = nix_editor::write::addtoarr(&current, "home.packages", arr_pkgs)
                    .map_err(|e| Error::NixEditor {
                        reason: e.to_string(),
                    })?;
            }
            let path = config
                .home_config_file
                .clone()
                .ok_or_else(|| Error::Config {
                    reason: "failed to get home config path".into(),
                })?;
            (current, path)
        }
    };

    let mut arguments = vec!["switch".to_string()];
    if let Ok(flakedir) = config.get_flake_dir() {
        arguments.push("--flake".to_string());
        arguments.push(flakedir);
    }

    Ok((content, output_path, arguments))
}

pub async fn install_spawn(
    pkgs: &[&str],
    md: &Metadata,
    auth_method: AuthMethod<'_>,
) -> Result<tokio::process::Child> {
    let config = configfile::get_config()?;
    let (content, output_path, _arguments) = prepare_install(pkgs, md, &config)?;

    let mut child = tokio::process::Command::new(if config.system_for_home_manager {
        match auth_method {
            AuthMethod::Dbus => unreachable!("D-Bus path handled in install()"),
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
