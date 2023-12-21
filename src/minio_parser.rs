use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use lazy_static::lazy_static;
use minio::s3::client::Client;
use minio::s3::creds::StaticProvider;
use minio::s3::http::BaseUrl;
use r2d2::Pool;
use crate::minio_parser;
use reqwest::Method;
use crate::{cache, config};
use crate::r2d2_minio::MinioConnectionManager;

lazy_static!(
    static ref MINIO_POOLS: Arc<Mutex<HashMap<String, Pool<MinioConnectionManager>>>> = {
        Arc::new(Mutex::new(HashMap::new()))
    };
);


fn get_config_by_json_key(config_key: &str) -> Option<config::MinioConfig> {
    config::WARP_MINIO_CONFIG.power.as_ref()?.get(config_key).cloned()
}

pub async fn get_generate_link_by_config_key_and_object_key(
    config_key: &str,
    key: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let config = match get_config_by_json_key(config_key) {
        Some(config) => {
            // 你现在可以访问与指定键相关联的配置
            config
        }
        None => {
            // 没有找到与指定键相关联的配置，或者 power 字段是 None
            let config_value = cache::get_minio_config(config_key)?.ok_or("Config not found")?;
            let cleaned_input = config_value.trim_matches('"').replace("\\\"", "\"");
            match serde_json::from_str::<config::MinioConfig>(&cleaned_input) {
                Ok(config) => config,
                Err(e) => {
                    eprintln!("Failed to parse JSON: {}", e);
                    return Err(Box::new(e)); // 返回一个错误
                }
            }
        }
    };

    let link = generate_minio_share_link(&config, key).await?;
    Ok(link)
}

async fn generate_minio_share_link(
    config_json: &config::MinioConfig,
    object: &str,
) -> Result<String, minio::s3::error::Error> {
    let pool = get_minio_client(&config_json.config_key);
    if pool.is_none() {
        return Err(minio::s3::error::Error::UrlBuildError("Failed to create client".to_string()));
    }
    let client = pool.unwrap().get().expect("Failed to get connection from pool");

    let args = minio::s3::args::GetPresignedObjectUrlArgs::new(&config_json.bucket_name, object, Method::GET)?;
    client.get_presigned_object_url(&args).await.map(|r| r.url)
}

fn get_minio_client(config_key: &str) ->Option<Pool<MinioConnectionManager>>{
    let pools = MINIO_POOLS.lock().unwrap();
    match pools.get(config_key) {
        Some(pool) => Some(pool.clone()),
        None => None,
    }

}

pub fn initialize_minio_pools() {
    let mut pools = match MINIO_POOLS.lock() {
        Ok(pools) => pools,
        Err(_) => {
            log::error!("Failed to lock MINIO_POOLS");
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
            continue;  // 如果池子已经存在，跳过当前迭代
        }

        let manager = MinioConnectionManager::new(
            config.endpoint.clone(),
            config.access_key.clone(),
            config.secret_key.clone()
        );

        let pool = match r2d2::Pool::builder().max_size(50).build(manager) {
            Ok(pool) => pool,
            Err(_) => {
                log::error!("Redis pool creation failed for key: {}", key);
                continue;
            }
        };

        pools.insert(key.clone(), pool);
    }
    log::info!("MinIO pools initialization completed");
}

