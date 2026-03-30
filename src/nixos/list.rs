use crate::{
    Error, Package, PackageAttr, Result,
    config::configfile::{self, ConfigMode},
    metadata::Metadata,
    toml as tomlcfg,
};

pub fn list_systempackages(md: &Metadata) -> Result<Vec<Package>> {
    let config = configfile::get_config()?;

    let attrs: Vec<String> = match config.mode {
        ConfigMode::Toml => {
            let path = tomlcfg::system_config_file_path()?;
            let pf = tomlcfg::read_system(std::path::Path::new(&path))?;
            let mut attrs = pf.packages;
            for key in pf.options.keys() {
                if let Some(rest) = key.strip_prefix("programs.")
                    && let Some(name) = rest.strip_suffix(".enable")
                    && !name.contains('.')
                    && !attrs.contains(&name.to_string())
                {
                    attrs.push(name.to_string());
                }
            }
            attrs
        }
        ConfigMode::Nix => {
            let content = config.read_system_config_file()?;
            let system_packages =
                nix_editor::read::getarrvals(&content, "environment.systemPackages").map_err(
                    |e| Error::NixEditor {
                        reason: e.to_string(),
                    },
                )?;
            let mut attrs: Vec<String> = system_packages
                .iter()
                .map(|x| x.strip_prefix("pkgs.").unwrap_or(x).to_string())
                .collect();
            for name in md.all_program_option_attrs() {
                let key = format!("programs.{}.enable", name);
                if let Ok(val) = nix_editor::read::readvalue(&content, &key)
                    && val.trim() == "true"
                    && !attrs.contains(&name)
                {
                    attrs.push(name);
                }
            }
            attrs
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
