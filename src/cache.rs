use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use lazy_static::lazy_static;
use r2d2::Pool;
use r2d2_redis::redis::{Commands, RedisResult};
use r2d2_redis::RedisConnectionManager;
use tokio::io::AsyncReadExt;

use crate::{cache, config};
use crate::config::redis_config::RedisConfig;
use crate::config::WARP_MINIO_CONFIG;

// 全局静态变量连接池
lazy_static! {
    static ref REDIS_POOLS: Arc<RwLock<HashMap<String, Pool<RedisConnectionManager>>>> = {
        Arc::new(RwLock::new(HashMap::new()))
    };
}

pub fn initialize_redis_pools() {

    // default config
    match &WARP_MINIO_CONFIG.default.redis_config {
        None => log::info!("Redis default config is None"),
        Some(vcr) => {
            insert_pool(vcr);
        }
    }

    insert_pool(&WARP_MINIO_CONFIG.power_redis_configs());

    log::info!("Redis pool initialization completed");
}

fn insert_pool(config_redis: &Vec<RedisConfig>) {
    let mut pools = REDIS_POOLS.write().unwrap();

    for config in config_redis {

        let manager = match RedisConnectionManager::new(config.redis_url()) {
            Ok(manager) => manager,
            Err(_) => {
                continue;
            }
        };

        let pool = match Pool::builder()
            .min_idle(config.idle_pool_size)
            .max_size(config.max_pool_size.unwrap_or(8))
            .build(manager) {
            Ok(pool) => pool,
            Err(_) => {
                log::error!("Redis pool creation failed for key: {}", config.pool_key());
                continue;
            }
        };
        pools.insert(format!("{}", config.pool_key()), pool);
    }
}


// 获取minio配置
pub fn get_minio_config(config_key: &str) -> RedisResult<Option<String>> {
    let pool = match cache::get_redis_pool(config_key) {
        Ok(pool) => pool,
        Err(_) => return Ok(None)
    };

    let mut con = pool
        .get()
        .expect("Failed to get connection from pool");
    let minio_config_key = format!("{}{}", config::MINIO_CONFIG_KEY_PREFIX, config_key);
    con.get(&minio_config_key)
}


pub fn get_redis_pool(key: &str) -> Result<Pool<RedisConnectionManager>, String> {
    let redis = WARP_MINIO_CONFIG.get_redis_by_config_key(key);
    let pools = REDIS_POOLS.read().map_err(|e| e.to_string())?;
    pools.get(&redis.unwrap().pool_key())
        .cloned()
        .ok_or_else(|| format!("No Redis pool found for key: {}", key))
}