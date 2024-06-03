use crate::{
    config::configfile::get_config,
    utils::{misc::get_pname_from_storepath, storedb::get_storebatch},
    Package, PackageAttr, HOME,
};
use anyhow::Result;
use rayon::prelude::*;

// nix-store --query --references ~/.local/state/home-manager/gcroots/current-home/home-path
pub async fn list_references() -> Result<Vec<Package>> {
    let output = std::process::Command::new("nix-store")
        .arg("--query")
        .arg("--references")
        .arg(&format!(
            "{}/.local/state/home-manager/gcroots/current-home/home-path",
            &*HOME
        ))
        .output()?;
    let stdout = String::from_utf8(output.stdout)?;
    let store_paths = stdout
        .split("\n")
        .filter(|x| !x.is_empty())
        .collect::<Vec<_>>();

    let mut names = store_paths
        .par_iter()
        .map(|x| x.split('/').last().unwrap_or(&x))
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

// List all packages in `home.packages`
pub fn list(db: &rusqlite::Connection) -> Result<Vec<Package>> {
    let config = get_config()?;

    let home_config = config.read_home_config_file()?;
    let home_packages = nix_editor::read::getarrvals(&home_config, "home.packages")?;

    let pkgs = home_packages
        .iter()
        .map(|x| x.strip_prefix("pkgs.").unwrap_or(x))
        .collect::<Vec<_>>();

    let mut stmt: rusqlite::Statement =
        db.prepare("SELECT pname, version FROM pkgs WHERE attribute = ?")?;
    let mut packages = Vec::new();
    for pkg in &pkgs {
        let mut rows = stmt.query(&[pkg])?;
        while let Some(row) = rows.next()? {
            let pname: String = row.get(0)?;
            let version: String = row.get(1)?;
            packages.push(Package {
                attr: PackageAttr::NixPkgs {
                    attr: pkg.to_string(),
                },
                version: if !version.is_empty() {
                    Some(version)
                } else {
                    None
                },
                pname: Some(pname),
                ..Default::default()
            });
        }
    }
    Ok(packages)
}
