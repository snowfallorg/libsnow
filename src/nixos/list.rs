use anyhow::Result;

use crate::{Package, PackageAttr, config::configfile::get_config, metadata::Metadata};

pub fn list_systempackages(md: &Metadata) -> Result<Vec<Package>> {
    let config = get_config()?;
    let system_packages = nix_editor::read::getarrvals(
        &config.read_system_config_file()?,
        "environment.systemPackages",
    )?;
    let pkgs = system_packages
        .iter()
        .map(|x| x.strip_prefix("pkgs.").unwrap_or(x).to_string())
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
