use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct BatchStoreRequest<'a> {
    stores: Vec<&'a str>,
    includestore: bool,
}

#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub struct BatchStoreResponse {
    pub packages: Vec<StoreResponse>,
}

#[derive(Debug, Deserialize)]
pub struct StoreResponse {
    pub attribute: Vec<String>,
    pub version: Option<String>,
    pub store: String,
}

impl BatchStoreResponse {
    pub fn get_attributes(&self) -> Vec<String> {
        self.packages
            .iter()
            .map(|x| x.attribute.to_vec())
            .flatten()
            .collect()
    }
}

pub async fn get_storebatch(stores: Vec<&str>) -> Result<BatchStoreResponse> {
    Ok(reqwest::Client::new()
        .post("https://api.snowflakeos.org/v0/storebatch")
        .json(&BatchStoreRequest {
            stores,
            includestore: true,
        })
        .send()
        .await?
        .json::<BatchStoreResponse>()
        .await?)
}
