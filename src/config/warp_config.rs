use std::collections::HashMap;

use serde::Deserialize;

use crate::auth::AuthType;
use crate::config::default_config::DefaultConfig;
use crate::config::power_config::PowerConfig;
use crate::config::redis_config::RedisConfig;

#[derive(Deserialize, Debug, Default)]
pub struct WarpConfig {
    #[serde(rename = "server-port")]
    pub(crate) server_port: Option<u16>,
    #[serde(rename = "auth-type")]
    pub(crate) auth_type: Option<AuthType>,
    #[serde(rename = "match-prefix")]
    pub(crate) match_prefix: Option<String>,
    #[serde(rename = "parsing-content-type")]
    pub(crate) parsing_content_type: bool,
    #[serde(rename = "power")]
    pub(crate) power: Option<HashMap<String, PowerConfig>>,
    #[serde(rename = "default")]
    pub(crate) default: DefaultConfig
}

impl WarpConfig {
    pub fn power_redis_configs(&self) -> Vec<RedisConfig> {
        let mut redis_configs = Vec::new();
        if let Some(power) = &self.power {
            for power_config in power.values() {
                if let Some(redis_config_list) = &power_config.redis_config {
                    for redis_config in redis_config_list {
                        // 使用 clone() 来创建 RedisConfig 实例的拷贝
                        redis_configs.push(redis_config.clone());
                    }
                }
            }
        }
        redis_configs
    }

    pub fn bucket_name(&self, config_key: String) -> Option<String> {
        if let Some(power) = &self.power {
           if let Some(config) = power.get(&config_key) {
               return config.bucket_name.clone()
            }
        }
        None
    }
}
