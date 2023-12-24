use serde::Deserialize;

use crate::config::minio_config::MinioConfig;
use crate::config::redis_config::RedisConfig;

#[derive(Deserialize, Debug, Default)]
pub struct DefaultConfig {
    #[serde(rename = "bucket-name")]
    pub(crate) bucket_name: Option<String>,
    #[serde(rename = "redis-config")]
    pub(crate) redis_config: Option<Vec<RedisConfig>>,
    #[serde(rename = "minio-config")]
    pub(crate) minio_config: Option<Vec<MinioConfig>>,
}
