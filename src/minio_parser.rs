use minio::s3::client::Client;
use minio::s3::creds::StaticProvider;
use minio::s3::http::BaseUrl;
use reqwest::Method;
use crate::{cache, config};

fn get_config_by_json_key(config_key: &str) -> Option<config::MinioConfig> {
    if let Some(power_map) = &config::WARP_MINIO_CONFIG.power {
        return power_map.get(config_key).cloned(); // 使用 cloned 方法来克隆 MinioConfig
    }
    None
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

// 生成分享链接
async fn generate_minio_share_link(
    config_json: &config::MinioConfig,
    object: &str,
) -> Result<String, minio::s3::error::Error> {
    // 假设 WARP_MINIO_CONFIG.minio_base_url 是 Option<String> 类型

    let client =  get_minio_client(
        config_json.access_key.as_str(),
        config_json.secret_key.as_str(),
        config_json.endpoint.as_str(),
    );

    let bucket_name = &config_json.bucket_name;

    let args = minio::s3::args::GetPresignedObjectUrlArgs::new(bucket_name, object, Method::GET)
        .map_err(|e| {
            minio::s3::error::Error::UrlBuildError(format!("Failed to create args: {}", e))
        })?;

    let r = client.unwrap().get_presigned_object_url(&args).await.map_err(|e| {
        minio::s3::error::Error::UrlBuildError(format!("Failed to get presigned url: {}", e))
    })?;

    Ok(r.url)
}

fn get_minio_client(access_key: &str, secret_key: &str, endpoint: &str) -> Option<Client> {
    let base_url: Result<BaseUrl, _> = endpoint.parse();
    // 解析失败返回空
    if base_url.is_err() {
        return None;
    }
    // 获取BaseUrl
    let provider = StaticProvider::new(access_key, secret_key, None);
    let client = Client::new(base_url.unwrap(), Some(Box::new(provider)), None, None)
        .map_err(|e| format!("Failed to create client: {}", e))
        .ok()?;
    Some(client)
}


// fn initialize_minio_pools() -> Result<(), String> {
//     let mut pools = MINIO_POOLS.lock().unwrap();
//
//     for (key, config) in WARP_MINIO_CONFIG.power.as_ref().ok_or("Power is None")? {
//         if pools.get(key).is_none() {
//             let provider = StaticProvider::new(
//                 config.access_key.as_str(),
//                 config.secret_key.as_str(),
//                 None,
//             );
//             let client = Client::new(
//                 config.endpoint.as_str(),
//                 Some(Box::new(provider)),
//                 None,
//                 None,
//             )
//             .map_err(|e| format!("Failed to create client: {}", e))?;
//             pools.insert(key.clone(), client);
//         }
//     }
//     Ok(())
// }
