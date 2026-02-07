use crate::{ICON_UPDATER_EXEC, Package, PackageAttr, PackageUpdate, metadata::Metadata};
use anyhow::{Context, Result};
use log::debug;

pub fn get_name_from_storepath(path: &str) -> Result<String> {
    let name = path.split('/').next_back().context("No name found")?;
    let name = name.split('-').skip(1).collect::<Vec<_>>().join("-");
    Ok(name)
}

fn get_pname_version(name: &str) -> Result<(String, Option<String>)> {
    let parts: std::str::Split<char> = name.split('-');
    let index = parts
        .clone()
        .enumerate()
        .find(|(_, x)| {
            x.chars().next().unwrap().is_numeric() || x == &"unstable" || x == &"nightly"
        })
        .map(|(i, _)| i);
    if let Some(index) = index {
        let pname = parts.clone().take(index).collect::<Vec<_>>().join("-");
        let mut version = parts.skip(index).collect::<Vec<_>>().join("-");
        for s in ["bin", "dev", "out", "debug"] {
            version = version
                .strip_suffix(&format!("-{}", s))
                .unwrap_or(&version)
                .to_string();
        }
        Ok((pname, Some(version)))
    } else {
        Ok((name.to_string(), None))
    }
}

pub fn get_pname_version_from_storepath(path: &str) -> Result<(String, Option<String>)> {
    let name = get_name_from_storepath(path)?;
    // Split hello-1.2.3 -> hello, 1.2.3
    get_pname_version(&name)
}

pub fn get_pname_from_storepath(path: &str, version: Option<String>) -> Result<String> {
    let name = get_name_from_storepath(path)?;
    // hello-1.2.3 -> hello: where version="1.2.3"
    let name = if let Some(version) = version {
        name.strip_suffix(format!("-{}", version).as_str())
            .context("No name found")?
    } else {
        &name
    };
    Ok(name.to_string())
}

pub fn get_version_from_storepath(path: &str, pname: &str) -> Result<String> {
    let name = get_name_from_storepath(path)?;
    // hello-1.2.3 -> 1.2.3: where pname="hello"
    let version = name
        .strip_prefix(&format!("{}-", pname))
        .context("No version found")?;
    Ok(version.to_string())
}

pub async fn updatable(installed: Vec<Package>) -> Result<Vec<PackageUpdate>> {
    let mut updatable = vec![];
    let new_md = Metadata::connect_latest().await?;

    for pkg in installed {
        match &pkg.attr {
            PackageAttr::NixPkgs { attr } => {
                if let Ok(info) = new_md.get(attr)
                    && let Ok((_pname, Some(version))) =
                        get_pname_version(&format!("{}-{}", info.pname, info.version))
                {
                    debug!(
                        "{}: {} -> {}",
                        attr,
                        pkg.version.clone().unwrap_or_default(),
                        version
                    );
                    if version.is_empty() {
                        continue;
                    } else if pkg.version.is_none() || version != *pkg.version.as_ref().unwrap() {
                        updatable.push(PackageUpdate {
                            attr: pkg.attr.clone(),
                            new_version: version.to_string(),
                            old_version: pkg.version.unwrap_or_default(),
                        });
                    }
                }
            }
            PackageAttr::External { url: _, attr: _ } => {}
        }
    }
    Ok(updatable)
}

pub fn refresh_icons() -> Result<()> {
    let output = std::process::Command::new(ICON_UPDATER_EXEC).output()?;
    debug!("{}", String::from_utf8(output.stdout)?);
    Ok(())
}
