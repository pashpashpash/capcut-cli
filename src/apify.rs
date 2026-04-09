use std::{fs, path::Path};

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

const API_BASE: &str = "https://api.apify.com/v2";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ActorRun {
    pub id: String,
    pub status: Option<String>,
    #[serde(rename = "defaultDatasetId")]
    pub default_dataset_id: String,
    #[serde(rename = "defaultKeyValueStoreId")]
    pub default_key_value_store_id: Option<String>,
    #[serde(rename = "startedAt")]
    pub started_at: Option<String>,
    #[serde(rename = "finishedAt")]
    pub finished_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ActorRunResponse {
    data: ActorRun,
}

pub fn build_client() -> Result<Client> {
    Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .context("failed to build HTTP client")
}

pub fn run_actor(client: &Client, token: &str, actor_id: &str, input: &Value) -> Result<ActorRun> {
    let response = client
        .post(format!("{API_BASE}/acts/{actor_id}/runs?waitForFinish=300"))
        .bearer_auth(token)
        .json(input)
        .send()
        .with_context(|| format!("failed to run actor {actor_id}"))?
        .error_for_status()
        .with_context(|| format!("actor run failed for {actor_id}"))?;

    Ok(response
        .json::<ActorRunResponse>()
        .with_context(|| format!("failed to parse actor run response for {actor_id}"))?
        .data)
}

pub fn fetch_dataset_items<T: DeserializeOwned>(
    client: &Client,
    token: &str,
    dataset_id: &str,
) -> Result<Vec<T>> {
    let response = client
        .get(format!(
            "{API_BASE}/datasets/{dataset_id}/items?clean=true&format=json"
        ))
        .bearer_auth(token)
        .send()
        .with_context(|| format!("failed to fetch dataset {dataset_id}"))?
        .error_for_status()
        .with_context(|| format!("dataset fetch failed for {dataset_id}"))?;

    response
        .json::<Vec<T>>()
        .with_context(|| format!("failed to parse dataset {dataset_id}"))
}

pub fn fetch_dataset_values(client: &Client, token: &str, dataset_id: &str) -> Result<Vec<Value>> {
    fetch_dataset_items(client, token, dataset_id)
}

pub fn download_to_path(client: &Client, token: &str, url: &str, path: &Path) -> Result<()> {
    let request = if url.starts_with(API_BASE) {
        client.get(url).bearer_auth(token)
    } else {
        client.get(url)
    };

    let bytes = request
        .send()
        .with_context(|| format!("failed to download {url}"))?
        .error_for_status()
        .with_context(|| format!("download failed for {url}"))?
        .bytes()
        .with_context(|| format!("failed to read bytes from {url}"))?;

    fs::write(path, &bytes).with_context(|| format!("failed to write {}", path.display()))
}
