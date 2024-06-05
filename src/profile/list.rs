use crate::{utils::misc::get_pname_version_from_storepath, Package, PackageAttr, NIXARCH};
use anyhow::{Context, Result};
use log::debug;
use serde::Deserialize;
use std::collections::HashMap;

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
    let profileroot: ProfilePkgsRoot = serde_json::from_reader(
        std::process::Command::new("nix")
            .arg("profile")
            .arg("list")
            .arg("--json")
            .output()?
            .stdout
            .as_slice(),
    )?;

    let mut pkgs = Vec::new();
    for (profile_name, pkg) in profileroot.elements {
        if let (Some(attrpath), Some(originalurl)) = (pkg.attrpath, pkg.originalurl) {
            let storepath = pkg.storepaths[0].clone();

            debug!(
                "Listing package: {} {} {}",
                attrpath, originalurl, storepath
            );
            // let derivation = get_drv(&storepath)?;

            let (pname, version) = get_pname_version_from_storepath(&storepath)?;

            if let Some(pkgattr) =
                attrpath.strip_prefix(&format!("legacyPackages.{}.", NIXARCH.as_str()))
            {
                pkgs.push(Package {
                    attr: PackageAttr::NixPkgs {
                        attr: pkgattr.to_string(),
                    },
                    version: version,
                    pname: Some(pname),
                    profile_name: Some(profile_name),
                });
            } else {
                pkgs.push(Package {
                    attr: PackageAttr::External {
                        url: originalurl,
                        attr: attrpath,
                    },
                    version: version,
                    pname: Some(pname),
                    profile_name: Some(profile_name),
                });
            };
        }
    }

    return Ok(pkgs);
}

pub fn name_from_attr(attr: &str) -> Result<String> {
    let list = list()?;
    for pkg in list {
        match pkg.attr {
            PackageAttr::NixPkgs { attr: x } => {
                if x == attr {
                    return Ok(pkg.profile_name.context("Profile name not found")?);
                }
            }
            PackageAttr::External {
                url,
                attr: ext_attr,
            } => {
                if attr == format!("{}#{}", url, ext_attr)
                    || (ext_attr.ends_with(".default") && url == attr)
                {
                    return Ok(pkg.profile_name.context("Profile name not found")?);
                }
            }
        }
    }
    return Err(anyhow::anyhow!("Package not found"));
}
