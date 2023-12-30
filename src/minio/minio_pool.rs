use std::collections::HashMap;

use std::sync::Arc;
use std::time::Duration;

use lazy_static::lazy_static;
use log::{log, warn};
use minio::s3::args::ListBucketsArgs;
use r2d2::Pool;
use tokio::sync::RwLock;
use tokio::time::interval;

use crate::config::minio_config::MinioConfig;
use crate::config::WARP_MINIO_CONFIG;
use crate::minio::r2d2_minio::MinioConnectionManager;

lazy_static!(
     pub static ref MINIO_POOLS: Arc<RwLock<HashMap<String, Vec<MinioPoolInstance>>>> = {
        Arc::new(RwLock::new(HashMap::new()))
     };
);


pub struct MinioPoolInstance {
    pub(crate) pool: Pool<MinioConnectionManager>,
    pub(crate) is_healthy: bool,
}

pub struct MinioPool {
    instances: RwLock<HashMap<String, MinioPoolInstance>>,
    // 用于轮询的当前索引
    current_index: RwLock<usize>,
}

impl MinioPool {
    pub async fn perform_health_checks() {
        let mut interval = interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            let mut pools = MINIO_POOLS.write().unwrap(); // 获取写锁

            for pool_instances in pools.values_mut() {
                for instance in pool_instances {
                    let client = match instance.pool.get() {
                        Ok(client) => client,
                        Err(_) => {
                            instance.is_healthy = false;
                            continue;
                        }
                    };

                    instance.is_healthy = client.list_buckets(&ListBucketsArgs::default()).await.is_ok();
                }
            }
        }
    }

    pub async fn get_minio_client(config_key: &str) -> Option<Pool<MinioConnectionManager>> {
        let pools = MINIO_POOLS.read().await;
        if let Some(pool_instances) = pools.get(config_key) {
            // 实现轮询逻辑，选择一个健康的实例
            for instance in pool_instances.iter().cycle() {
                if instance.is_healthy {
                    return Some(instance.pool.clone());
                }
            }
        }
        None
    }
}


pub async fn initialize_minio_pools() {
    match &WARP_MINIO_CONFIG.default.minio_config {
        None => log::info!("Minio default config is None"),
        Some(configs) => {
            insert_pool( String::from("default"), &configs).await;
        }
    }

    match &WARP_MINIO_CONFIG.power {
        None => log::info!("Power config is None"),
        Some(power) => {
            for power_key in power.keys() {
                let power_value = power.get(power_key);
                if let Some(minio_configs) = &power_value.unwrap().minio_config {
                    insert_pool(power_key.to_string(), minio_configs).await;
                }
            }
        }
    }

    log::info!("MinIO pools initialization completed");
}


async fn insert_pool(config_key: String, configs: &Vec<MinioConfig>) {
    let mut pools = MINIO_POOLS.write();

    let mut pool_instances = Vec::new();
    for config in configs {
        let manager = MinioConnectionManager::new(
            config.endpoint.clone(),
            config.access_key.clone(),
            config.secret_key.clone(),
        );
        let pool = Pool::builder()
            .min_idle(config.idle_pool_size)
            .max_size(config.max_pool_size.unwrap_or(8))
            .build(manager)
            .expect("Failed to create MinIO pool");

        pool_instances.push(MinioPoolInstance { pool, is_healthy: true });
    }
    pools.await.insert(config_key, pool_instances);
}

