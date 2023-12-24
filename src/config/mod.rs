use std::{env, fs};
use std::path::Path;

use lazy_static::lazy_static;

use crate::config::warp_config::WarpConfig;

pub mod redis_config;
pub mod power_config;
pub mod minio_config;
pub mod default_config;
pub mod warp_config;


// 环境变量名称
const CONFIG_PATH_KEY: &'static str = "WARP_MINIO_CONFIG_PATH";

pub const BEARER_PREFIX_LEN: usize = 7;

// 文件路径前缀
pub const URL_PREFIX: &'static str = "/minio";

// 配置路径
pub const MINIO_CONFIG_KEY_PREFIX: &'static str = "sys_oss:";

// 端口
pub const PORT: u16 = 9928;

lazy_static! {
  pub static ref WARP_MINIO_CONFIG: WarpConfig = {
        let config_path = env::var(CONFIG_PATH_KEY).unwrap_or_else(|_| "config.yaml".to_string());
        let path = Path::new(&config_path);
        let config = fs::read_to_string(path)
            .map_err(|e| eprintln!("Failed to read config file: {}", e))
            .and_then(|content| serde_yaml::from_str(&content)
                .map_err(|e| eprintln!("Failed to parse config file: {}", e)))
            .unwrap_or_else(|_| WarpConfig::default());
        config
    };
}

