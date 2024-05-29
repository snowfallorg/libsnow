use crate::{
    nixenv::list::list, utils, PackageUpdate
};
use anyhow::Result;

pub async fn updatable() -> Result<Vec<PackageUpdate>> {
    utils::misc::updatable(list().await?).await
}
