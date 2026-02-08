use anyhow::{Context, Result};

use crate::{
    config::configfile::{self, ConfigMode},
    metadata::Metadata,
    toml as tomlcfg, Package, PackageAttr,
};

pub fn list(md: &Metadata) -> Result<Vec<Package>> {
    let config = configfile::get_config()?;

    let attrs: Vec<String> = match config.mode {
        ConfigMode::Toml => {
            let user = tomlcfg::current_user()?;
            let path = tomlcfg::config_file_path()?;
            let pf = tomlcfg::read(std::path::Path::new(&path))?;
            pf.home
                .get(&user)
                .context("No home section for user")?
                .packages
                .clone()
        }
        ConfigMode::Nix => {
            let home_config = config.read_home_config_file()?;
            let home_packages = nix_editor::read::getarrvals(&home_config, "home.packages")?;
            home_packages
                .iter()
                .map(|x| x.strip_prefix("pkgs.").unwrap_or(x).to_string())
                .collect()
        }
    };

    let mut packages = Vec::new();
    for attr in &attrs {
        if let Ok(info) = md.get(attr) {
            packages.push(Package {
                attr: PackageAttr::NixPkgs {
                    attr: attr.to_string(),
                },
                version: if !info.version.is_empty() {
                    Some(info.version)
                } else {
                    None
                },
                pname: Some(info.pname),
                ..Default::default()
            });
        } else {
            packages.push(Package {
                attr: PackageAttr::NixPkgs {
                    attr: attr.to_string(),
                },
                ..Default::default()
            });
        }
    }
    Ok(packages)
}
