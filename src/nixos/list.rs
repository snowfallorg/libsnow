use anyhow::Result;
use rayon::prelude::*;

use crate::{
    Package, PackageAttr,
    config::configfile::get_config,
    metadata::Metadata,
    utils::{misc::get_pname_from_storepath, storedb::get_storebatch},
};

// curl https://api.snowflakeos.org/v0/storebatch -X POST -d '{"stores": ["x1", "x2"]}'
pub async fn list_references() -> Result<Vec<Package>> {
    let output = std::process::Command::new("nix-store")
        .arg("--query")
        .arg("--references")
        .arg("/run/current-system/sw")
        .output()?;
    let stdout = String::from_utf8(output.stdout)?;
    let store_paths = stdout
        .split('\n')
        .filter(|x| !x.is_empty())
        .collect::<Vec<_>>();

    let mut names = store_paths
        .par_iter()
        .map(|x| x.split('/').next_back().unwrap_or(x))
        .collect::<Vec<_>>();
    names.sort();

    // TOOD: Add local caching using

    let storebatch = get_storebatch(names).await?;
    Ok(storebatch
        .packages
        .into_iter()
        .map(|x| Package {
            attr: PackageAttr::NixPkgs {
                attr: x.attribute.join("."),
            },
            version: x.version.clone(),
            pname: get_pname_from_storepath(x.store.as_str(), x.version).ok(),
            ..Default::default()
        })
        .collect())
}

// List all packages in `enviroment.systemPackages`
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

// List all pacakges in nsc toml file
pub fn list_tomlpackages() -> Result<Vec<String>> {
    unimplemented!()
}
