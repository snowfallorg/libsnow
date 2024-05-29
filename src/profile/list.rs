use crate::{utils::misc::get_pname_version_from_storepath, Package, PackageAttr, NIXARCH};
use anyhow::Result;
use serde::Deserialize;
use std::{collections::HashMap, fs::File};

#[derive(Debug, Deserialize)]
struct ProfilePkgsRoot {
    elements: HashMap<String, ProfilePkgOut>,
}

#[derive(Debug, Deserialize)]
struct ProfilePkgOut {
    #[serde(rename = "attrPath")]
    attrpath: Option<String>,
    #[serde(rename = "originalUrl")]
    originalurl: Option<String>,
    #[serde(rename = "storePaths")]
    storepaths: Vec<String>,
}

pub fn list() -> Result<Vec<Package>> {
    let profileroot: ProfilePkgsRoot = serde_json::from_reader(File::open(&format!(
        "{}/.nix-profile/manifest.json",
        std::env::var("HOME")?
    ))?)?;

    let mut pkgs = Vec::new();
    for (_, pkg) in profileroot.elements {
        if let (Some(attrpath), Some(originalurl)) = (pkg.attrpath, pkg.originalurl) {
            let storepath = pkg.storepaths[0].clone();

            println!("{} {} {}", attrpath, originalurl, storepath);
            // let derivation = get_drv(&storepath)?;

            let (pname, version) = get_pname_version_from_storepath(&storepath)?;

            if let Some(pkgattr) =
                attrpath.strip_prefix(&format!("legacyPackages.{}.", NIXARCH.as_str()))
            {
                pkgs.push(Package {
                    attr: PackageAttr::NixPkgs {
                        attr: pkgattr.to_string(),
                    },
                    version: Some(version),
                    pname: Some(pname),
                    // store: Some(PathBuf::from(storepath)),
                });
            } else {
                pkgs.push(Package {
                    attr: PackageAttr::External {
                        url: originalurl,
                        attr: attrpath,
                    },
                    version: Some(version),
                    pname: Some(pname),
                    // store: Some(PathBuf::from(storepath)),
                });
            };
        }
    }

    return Ok(pkgs);
}
