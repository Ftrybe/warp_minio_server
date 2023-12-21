use std::collections::HashMap;

use lazy_static::lazy_static;


use std::{env, fs};
use std::path::Path;
use std::string::String;

use serde::Deserialize;


// 环境变量名称
const CONFIG_PATH_KEY: &'static str = "WARP_MINIO_CONFIG_PATH";

pub const BEARER_PREFIX_LEN: usize = 7;

// redis数据库路径 9为选择数据库
pub const REDIS_URL: &'static str = "redis://127.0.0.1/0";

// 文件路径前缀
pub const URL_PREFIX: &'static str = "/minio";

// 配置路径
pub const MINIO_CONFIG_KEY_PREFIX: &'static str = "sys_oss:";

// 端口
pub const PORT: u16 = 9928;

lazy_static! {
      pub static ref WARP_MINIO_CONFIG: WarpMinioConfig = {
        let config_path = env::var(CONFIG_PATH_KEY).unwrap_or_else(|_| "config.json".to_string());
        let path = Path::new(&config_path);
        let config = fs::read_to_string(path)
            .map_err(|e| eprintln!("Failed to read config file: {}", e))
            .and_then(|content| serde_json::from_str(&content)
                .map_err(|e| eprintln!("Failed to parse config file: {}", e)))
            .unwrap_or_else(|_| WarpMinioConfig::default());
        config
    };
}



#[derive(Deserialize, Debug, Clone)]
pub struct MinioConfig {
    #[serde(rename = "configKey")]
    pub(crate) config_key: String,
    #[serde(rename = "accessKey")]
    pub(crate) access_key: String,
    #[serde(rename = "secretKey")]
    pub(crate) secret_key: String,
    #[serde(rename = "bucketName")]
    pub(crate) bucket_name: String,
    #[serde(rename = "endpoint")]
    pub(crate) endpoint: String,
    #[serde(rename = "redisUrl")]
    pub(crate) redis_url: Option<String>,
}

#[derive(Deserialize, Debug, Default)]
pub struct WarpMinioConfig {
    #[serde(rename = "serverPort")]
    pub(crate) server_port: Option<u16>,
    #[serde(rename = "disabledAuth")]
    pub(crate) disabled_auth: bool,
    #[serde(rename = "defaultRedisUrl")]
    pub(crate) default_redis_url: Option<String>,
    #[serde(rename = "matchPrefix")]
    pub(crate) match_prefix: Option<String>,
    #[serde(rename = "reParsingContentType")]
    pub(crate) re_parsing_content_type: bool,
    #[serde(rename = "power")]
    pub(crate)  power: Option<HashMap<String, MinioConfig>>,
}
