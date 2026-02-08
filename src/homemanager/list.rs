use crate::{Package, PackageAttr, config::configfile::get_config, metadata::Metadata};
use anyhow::Result;

pub fn list(md: &Metadata) -> Result<Vec<Package>> {
    let config = get_config()?;

    let home_config = config.read_home_config_file()?;
    let home_packages = nix_editor::read::getarrvals(&home_config, "home.packages")?;

    let pkgs = home_packages
        .iter()
        .map(|x| x.strip_prefix("pkgs.").unwrap_or(x))
        .collect::<Vec<_>>();

    let mut packages = Vec::new();
    for pkg in &pkgs {
        if let Ok(info) = md.get(pkg) {
            packages.push(Package {
                attr: PackageAttr::NixPkgs {
                    attr: pkg.to_string(),
                },
                version: if !info.version.is_empty() {
                    Some(info.version)
                } else {
                    None
                },
                pname: Some(info.pname),
                ..Default::default()
            });
        }
    }
    Ok(packages)
}
