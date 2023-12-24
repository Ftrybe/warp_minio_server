use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct MinioConfig {
    #[serde(rename = "access-key")]
    pub(crate) access_key: String,
    #[serde(rename = "secret-key")]
    pub(crate) secret_key: String,
    #[serde(rename = "endpoint")]
    pub(crate) endpoint: String,
    #[serde(rename = "max-pool-idle")]
    pub(crate) max_pool_size: Option<u32>,
    #[serde(rename = "idle-pool-size")]
    pub(crate) idle_pool_size: Option<u32>,
}

