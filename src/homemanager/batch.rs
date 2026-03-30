use crate::{
    Error, Result,
    config::configfile::{self, ConfigMode},
    homemanager::list::list,
    metadata::Metadata,
    toml as tomlcfg,
};
use toml::Value as TomlValue;
use tracing::debug;

pub fn prepare(installs: &[&str], removes: &[&str], md: &Metadata) -> Result<String> {
    let config = configfile::get_config()?;

    let installed: Vec<String> = list(md)?.into_iter().map(|x| x.attr.to_string()).collect();

    let mut pkgs_to_install = vec![];
    for pkg in installs {
        if md.get(pkg).is_ok() {
            if installed.contains(&pkg.to_string()) {
                debug!("{} is already installed", pkg);
            } else {
                pkgs_to_install.push(pkg.to_string());
            }
        }
    }

    let mut pkgs_to_remove = vec![];
    for pkg in removes {
        if md.get(pkg).is_ok() {
            if installed.contains(&pkg.to_string()) {
                pkgs_to_remove.push(pkg.to_string());
            } else {
                debug!("{} is not installed", pkg);
            }
        }
    }

    if pkgs_to_install.is_empty() && pkgs_to_remove.is_empty() {
        return Err(Error::NothingToDo {
            reason: "no packages to install or remove".into(),
        });
    }

    let content = match config.mode {
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

            section.packages.sort();
            toml::to_string_pretty(&pf)?
        }
        ConfigMode::Nix => {
            let mut current = config.read_home_config_file()?;

            let mut install_arr_pkgs = vec![];
            for attr in &pkgs_to_install {
                if md.has_hm_program_option(attr) {
                    let key = format!("programs.{}.enable", attr);
                    current = nix_editor::write::write(&current, &key, "true").map_err(|e| {
                        Error::NixEditor {
                            reason: e.to_string(),
                        }
                    })?;
                } else {
                    install_arr_pkgs.push(attr.clone());
                }
            }
            if !install_arr_pkgs.is_empty() {
                if let Ok(withvals) = nix_editor::read::getwithvalue(&current, "home.packages")
                    && !withvals.contains(&String::from("pkgs"))
                {
                    install_arr_pkgs = install_arr_pkgs
                        .iter()
                        .map(|x| format!("pkgs.{}", x))
                        .collect();
                }
                current = nix_editor::write::addtoarr(&current, "home.packages", install_arr_pkgs)
                    .map_err(|e| Error::NixEditor {
                        reason: e.to_string(),
                    })?;
            }

            let mut remove_arr_pkgs = vec![];
            for attr in &pkgs_to_remove {
                let key = format!("programs.{}.enable", attr);
                if nix_editor::read::readvalue(&current, &key).is_ok() {
                    current =
                        nix_editor::write::deref(&current, &key).map_err(|e| Error::NixEditor {
                            reason: e.to_string(),
                        })?;
                } else {
                    remove_arr_pkgs.push(attr.clone());
                }
            }
            if !remove_arr_pkgs.is_empty() {
                if let Ok(withvals) = nix_editor::read::getwithvalue(&current, "home.packages")
                    && !withvals.contains(&String::from("pkgs"))
                {
                    remove_arr_pkgs = remove_arr_pkgs
                        .iter()
                        .map(|x| format!("pkgs.{}", x))
                        .collect();
                }
                current = nix_editor::write::rmarr(&current, "home.packages", remove_arr_pkgs)
                    .map_err(|e| Error::NixEditor {
                        reason: e.to_string(),
                    })?;
            }

            current
        }
    };

    Ok(content)
}
