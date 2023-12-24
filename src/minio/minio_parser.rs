use std::collections::HashMap;

use lazy_static::lazy_static;
use reqwest::Method;
use tokio::sync::RwLock;
use crate::config::WARP_MINIO_CONFIG;

use crate::minio::minio_pool::MinioPool;

lazy_static!(
    static ref MINIO_KET_TO_BUCKET_MAP: RwLock<HashMap<String, String>> = {
        RwLock::new(HashMap::new())
    };
);


pub async fn get_generate_link_by_config_key_and_object_key(
    minio_config_key: &str,
    object_key: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {

    let bucket_name = get_minio_bucket_by_minio_config_key(minio_config_key).await.unwrap_or_else(|| String::from(""));

    let link = generate_minio_share_link(minio_config_key, &bucket_name, object_key).await?;
    return Ok(link)

}

async fn generate_minio_share_link(
    config_key: &str,
    bucket_name: &str,
    object: &str,
) -> Result<String, minio::s3::error::Error> {
    let pool = MinioPool::get_minio_client(config_key).await
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "No available MinIO client found")
        })?;
    let client = pool.get().expect("Failed to get connection from pool");
    let args = minio::s3::args::GetPresignedObjectUrlArgs::new(bucket_name, object, Method::GET)?;
    client.get_presigned_object_url(&args).await.map(|r| r.url)
}



async fn get_minio_bucket_by_minio_config_key(config_key: &str) -> Option<String> {
    let read_map = MINIO_KET_TO_BUCKET_MAP.read();
    if let Some(name) = read_map.await.get(config_key) {
        return Some(name.clone());
    }

    // 由于 WARP_MINIO_CONFIG.bucket_name 没有提供实现细节，这里假设它返回 Option<String>
    if let Some(bucket_name) = WARP_MINIO_CONFIG.bucket_name(config_key.to_string()) {
        let mut write_map = MINIO_KET_TO_BUCKET_MAP.write().await;
        write_map.insert(config_key.to_string(), bucket_name.clone());
        return Some(bucket_name);
    }

    None
}