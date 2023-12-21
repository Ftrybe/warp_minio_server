
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use lazy_static::lazy_static;
use r2d2_redis::redis::{Commands, RedisResult};
use r2d2_redis::RedisConnectionManager;
use r2d2::Pool;
use crate::config;

// 全局静态变量连接池
lazy_static! {
    static ref REDIS_POOLS: Arc<Mutex<HashMap<String, Pool<RedisConnectionManager>>>> = {
        Arc::new(Mutex::new(HashMap::new()))
    };
}

pub fn initialize_redis_pools() {
    let mut pools = match REDIS_POOLS.lock() {
        Ok(pools) => pools,
        Err(_) => {
            log::error!("Failed to lock REDIS_POOLS");
            return;
        }
    };

    let config_power = match config::WARP_MINIO_CONFIG.power.as_ref() {
        Some(power) => power,
        None => {
            log::error!("Power configuration is missing");
            return;
        }
    };

    for (key, config) in config_power {
        if pools.contains_key(key) {
            continue;  // 如果已存在相应的连接池，跳过当前迭代
        }

        let redis_url = match config.redis_url.as_deref() {
            Some(url) => url,
            None => {
                log::error!("No Redis URL specified for key: {}", key);
                continue;
            }
        };

        let manager = match RedisConnectionManager::new(redis_url) {
            Ok(manager) => manager,
            Err(_) => {
                log::error!("Redis connection manager creation failed for key: {}", key);
                continue;
            }
        };

        let pool = match r2d2::Pool::builder().max_size(50).build(manager) {
            Ok(pool) => pool,
            Err(_) => {
                log::error!("Redis pool creation failed for key: {}", key);
                continue;
            }
        };

        pools.insert(key.clone(), pool);
        log::info!("Redis pool created for key: {}", key);
    }

    log::info!("Redis pool initialization completed");
}

// 根据token 获取redis中的用户信息
pub fn fetch_user_info_from_redis(config_key: &str, token: &str) -> RedisResult<Option<String>> {
    // 从连接池获取连接
    let pool = get_redis_pool(config_key);
    if pool.is_none() {
        return Ok(None);
    }
    let mut con = pool
        .unwrap()
        .get()
        .expect("Failed to get connection from pool");
    let user_token_key = format!("Authorization:login:token:{}", token);
    // 使用连接执行Redis GET命令
    con.get(&user_token_key)
}

// 获取minio配置
pub fn get_minio_config(config_key: &str) -> RedisResult<Option<String>> {
    let pool = get_redis_pool(config_key);
    if pool.is_none() {
        return Ok(None);
    }
    let mut con = pool
        .unwrap()
        .get()
        .expect("Failed to get connection from pool");
    let minio_config_key = format!("{}{}", config::MINIO_CONFIG_KEY_PREFIX, config_key);
    con.get(&minio_config_key)
}


fn get_redis_pool(key: &str) -> Option<Pool<RedisConnectionManager>> {
    let pools = REDIS_POOLS.lock().unwrap();
    match pools.get(key) {
        Some(pool) => Some(pool.clone()),
        None => None,
    }
}