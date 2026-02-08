use crate::{
    HELPER_EXEC,
    config::configfile::{self, ConfigMode},
    homemanager::list::list,
    metadata::Metadata,
    toml as tomlcfg,
};
use anyhow::{Context, Result, anyhow};
use log::debug;
use tokio::io::AsyncWriteExt;
use toml::Value as TomlValue;

pub async fn install(pkgs: &[&str], md: &Metadata) -> Result<()> {
    let config = configfile::get_config()?;

    let installed: Vec<String> = list(md)
        .unwrap_or_default()
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
            let user = tomlcfg::current_user()?;
            let path = tomlcfg::config_file_path()?;
            let mut pf = tomlcfg::read(std::path::Path::new(&path))?;
            let section = pf.home.entry(user).or_default();
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
            let oldconfig = config.read_home_config_file()?;
            if let Ok(withvals) = nix_editor::read::getwithvalue(&oldconfig, "home.packages")
                && !withvals.contains(&String::from("pkgs"))
            {
                pkgs_to_install = pkgs_to_install
                    .iter()
                    .map(|x| format!("pkgs.{}", x))
                    .collect();
            }
            let newconfig =
                nix_editor::write::addtoarr(&oldconfig, "home.packages", pkgs_to_install)?;
            let path = config
                .homeconfig
                .clone()
                .context("Failed to get home config path")?;
            (newconfig, path)
        }
    };

    let mut output = tokio::process::Command::new(HELPER_EXEC)
        .arg("config-home")
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
