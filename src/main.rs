extern crate jsonwebtoken as jwt;
extern crate minio;
extern crate r2d2;
extern crate r2d2_redis;
extern crate redis;
extern crate serde_json;
extern crate url;

use crate::r2d2_redis::redis::Commands;


use bytes::Bytes;

use lazy_static::lazy_static;

use minio::s3::client::Client;
use minio::s3::creds::StaticProvider;
use minio::s3::http::BaseUrl;


use r2d2_redis::redis::RedisResult;
use r2d2_redis::RedisConnectionManager;
use r2d2::Pool;
use reqwest::Method;

use std::collections::HashMap;
use std::{env, fs};
use std::path::Path;
use std::string::String;
use std::sync::{Arc, Mutex};

use serde::Deserialize;

use warp::http::StatusCode;
use warp::{Filter, Rejection};

use mime_guess::from_path;


// 环境变量名称
const CONFIG_PATH_KEY: &'static str = "WARP_MINIO_CONFIG_PATH";

const BEARER_PREFIX_LEN: usize = 7;

// redis数据库路径 9为选择数据库
const REDIS_URL: &'static str = "redis://127.0.0.1/0";

// 文件路径前缀
const URL_PREFIX: &'static str = "/minio";

// 配置路径
const MINIO_CONFIG_KEY_PREFIX: &'static str = "sys_oss:";

// 端口
const PORT: u16 = 9928;

// 全局静态变量连接池
lazy_static! {

     static ref WARP_MINIO_CONFIG: WarpMinioConfig = {
        let config_path = env::var(CONFIG_PATH_KEY).unwrap_or_else(|_| "config.json".to_string());
        let path = Path::new(&config_path);
        let config = fs::read_to_string(path)
            .map_err(|e| eprintln!("Failed to read config file: {}", e))
            .and_then(|content| serde_json::from_str(&content)
                .map_err(|e| eprintln!("Failed to parse config file: {}", e)))
            .unwrap_or_else(|_| WarpMinioConfig::default());
        config
    };

    static ref CLIENT: reqwest::Client = reqwest::Client::builder()
        .build()
        .unwrap();

    static ref MINIO_POOLS: Arc<Mutex<HashMap<String, Client>>> = {
        Arc::new(Mutex::new(HashMap::new()))
    };

    static ref REDIS_POOLS: Arc<Mutex<HashMap<String, Pool<RedisConnectionManager>>>> = {
        Arc::new(Mutex::new(HashMap::new()))
    };
}

#[tokio::main]
async fn main() {
    env::set_var("RUST_LOG", "info");
    env_logger::init();
    let _ = initialize_redis_pools();

    let cors = warp::cors()
        .allow_any_origin();

    let optional_auth_header = warp::header::optional("authorization");

    let route = warp::path::full()
        .and(warp::query::<HashMap<String, String>>())
        .and(optional_auth_header)
        .and(warp::method())
        .and(warp::header::headers_cloned())
        .and(warp::body::bytes())
        .and_then(process)
        .with(cors);

    let mut server_port = PORT; // 用您的默认前缀替换此处

    if let Some(ref prefix) = WARP_MINIO_CONFIG.server_port {
        if *prefix != 0 {
            // 假设0是一个无效的端口号
            server_port = *prefix;
        }
    }

    log::info!(
        "Auth is disabled: {}",
        WARP_MINIO_CONFIG.disabled_auth.to_string()
    );

    warp::serve(route).run(([127, 0, 0, 1], server_port)).await;
}

async fn process(
    path: warp::path::FullPath,
    params: HashMap<String, String>,
    auth_header: Option<String>,
    _method: warp::http::Method,
    _headers: warp::http::HeaderMap,
    _body: bytes::Bytes,
) -> Result<Box<dyn warp::Reply>, Rejection> {
    // 在这里处理你的业务逻辑，例如与数据库交互，文件操作等
    let request_uri = path.as_str();

    let mut url_prefix = URL_PREFIX; // 用您的默认前缀替换此处

    // 检查 WARP_MINIO_CONFIG.match_prefix 是否不为空
    if let Some(ref prefix) = WARP_MINIO_CONFIG.match_prefix {
        if !prefix.is_empty() {
            url_prefix = prefix;
        }
    }
    if !request_uri.starts_with(url_prefix) {
        // 前缀不匹配，返回一个错误响应
        let reply = warp::reply::with_status(
            "URI does not start with the expected prefix",
            warp::http::StatusCode::BAD_REQUEST,
        );
        return Ok(Box::new(reply));
    }

    // 现在 url_prefix 有了新的值（如果 WARP_MINIO_CONFIG.match_prefix 不为空）
    let bucket_path = &request_uri[url_prefix.len()..];

    // 去除字符串开头的'/'
    let path = bucket_path.trim_start_matches('/');

    // 使用`splitn`方法将字符串分割为两部分
    let mut parts = path.splitn(2, '/');

    // 获取第一部分作为配置key
    let config_key = parts.next().unwrap_or("");

    // 获取参数信息，是否指定了文件名称

    // 获取第二部分文件key
    let key = parts.next().unwrap_or("");

    let filename = params.get("filename");

    log::info!("Access: {}", request_uri);
    if !WARP_MINIO_CONFIG.disabled_auth {
        // 验证授权头
        if let Some(auth_str) = auth_header.as_ref() {
            if !auth_str.starts_with("Bearer ") {
                return unauthorized_reply();
            }

            let token = match trim_bearer_prefix(auth_str) {
                Some(token) => token,
                None => {
                    return unauthorized_reply();
                }
            };

            match fetch_user_info_from_redis(config_key, token) {
                Ok(user_info) => {
                    if user_info.is_none() {
                        return unauthorized_reply();
                    }
                }
                Err(_e) => {
                    return unauthorized_reply();
                }
            }
        } else {
            // 处理缺少授权头的情况
            return unauthorized_reply();
        }
    }

    let link = match get_generate_link_by_config_key_and_object_key(config_key, key).await {
        Ok(token) => token,
        Err(_) => return unauthorized_reply(), // if there's an error, return early
    };

    let client_range_header = _headers.get("Range").cloned();

    let client_request = CLIENT.get(link);

    // 如果存在 Range 头，则在请求中设置它
    let client_request = if let Some(range_header) = client_range_header {
        client_request.header("Range", range_header)
    } else {
        client_request
    };

    match client_request.send().await {
        Ok(res) => {
            let status = res.status();

            // 获取响应头
            let headers = res.headers().clone();

            // 读取响应体
            let body_bytes = res.bytes().await.unwrap();

            // 创建新的响应
            let mut response = warp::http::Response::builder().status(status);

            // 将原始响应的头部信息复制到新的响应中
            for (key, value) in headers.iter() {
                response = response.header(key, value);
            }

            // 如果设置了重新解析 Content-Type，则获取 Content-Type
            if WARP_MINIO_CONFIG.re_parsing_content_type {
                let content_type = headers.get("Content-Type")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string())
                    .map(|ct| {
                        if ct == "application/octet-stream" {
                            // 当 Content-Type 是 application/octet-stream 时，使用文件扩展名推断
                            from_path(key).first_or_octet_stream().to_string()
                        } else {
                            ct
                        }
                    })
                    .unwrap_or_else(|| {
                        // 当找不到 Content-Type 时，使用文件扩展名推断
                        from_path(key).first_or_octet_stream().to_string()
                    });

                response = response.header("Content-Type", content_type);
            }

            // 检查是否有 filename 参数
            let content_disposition_header =
                filename.map(|filename| format!("attachment; filename=\"{}\"", filename));

            // 如果有 filename，设置 Content-Disposition 头
            if let Some(header_value) = content_disposition_header {
                response = response.header("Content-Disposition", header_value);
            }

            let response = response.body(body_bytes).unwrap();

            Ok(Box::new(response) as Box<dyn warp::Reply>)
        }
        Err(_) => unauthorized_reply(),
    }
}

fn unauthorized_reply() -> Result<Box<dyn warp::Reply>, Rejection> {
    let body = Bytes::from_static(b"{\"error\": \"Unauthorized\"}");
    let response = warp::http::Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .body(body)
        .unwrap();
    Ok(Box::new(response) as Box<dyn warp::Reply>)
}

fn trim_bearer_prefix(auth_header: &str) -> Option<&str> {
    if auth_header.len() < BEARER_PREFIX_LEN || &auth_header[..BEARER_PREFIX_LEN] != "Bearer " {
        None
    } else {
        Some(&auth_header[BEARER_PREFIX_LEN..])
    }
}

// 根据token 获取redis中的用户信息
fn fetch_user_info_from_redis(config_key: &str, token: &str) -> RedisResult<Option<String>> {
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
fn get_minio_config(config_key: &str) -> RedisResult<Option<String>> {
    let pool = get_redis_pool(config_key);
    if pool.is_none() {
        return Ok(None);
    }
    let mut con = pool
        .unwrap()
        .get()
        .expect("Failed to get connection from pool");
    let minio_config_key = format!("{}{}", MINIO_CONFIG_KEY_PREFIX, config_key);
    con.get(&minio_config_key)
}

fn get_config_by_json_key(config_key: &str) -> Option<MinioConfig> {
    if let Some(power_map) = &WARP_MINIO_CONFIG.power {
        return power_map.get(config_key).cloned(); // 使用 cloned 方法来克隆 MinioConfig
    }
    None
}

async fn get_generate_link_by_config_key_and_object_key(
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
            let config_value = get_minio_config(config_key)?.ok_or("Config not found")?;
            let cleaned_input = config_value.trim_matches('"').replace("\\\"", "\"");
            match serde_json::from_str::<MinioConfig>(&cleaned_input) {
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
    config_json: &MinioConfig,
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

fn get_redis_pool(key: &str) -> Option<Pool<RedisConnectionManager>> {
    let pools = REDIS_POOLS.lock().unwrap();
    match pools.get(key) {
        Some(pool) => Some(pool.clone()),
        None => None,
    }
}
fn initialize_redis_pools() -> Result<(), String> {
    let mut pools = REDIS_POOLS.lock().unwrap();

    for (key, configs) in WARP_MINIO_CONFIG.power.as_ref().ok_or("Power is None")? {
        // for minio_config in configs {
        if let Some(redis_url) = configs.redis_url.as_deref() {
            if pools.get(key).is_none() {
                let manager = RedisConnectionManager::new(redis_url)
                    .expect("Redis connection manager creation failed");
                let pool = r2d2::Pool::builder()
                    .max_size(50) // 设置最大连接数
                    .build(manager)
                    .expect("Redis pool creation failed");
                pools.insert(key.clone(), pool);
                log::info!("")
            }
        } else {
            log::error!("No Redis URL specified for key: {}", key);
        }
        // }
    }
    log::info!("Redis pool creation success!");
    Ok(())
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

#[derive(Deserialize, Debug, Clone)]
struct MinioConfig {
    #[serde(rename = "configKey")]
    config_key: String,
    #[serde(rename = "accessKey")]
    access_key: String,
    #[serde(rename = "secretKey")]
    secret_key: String,
    #[serde(rename = "bucketName")]
    bucket_name: String,
    #[serde(rename = "endpoint")]
    endpoint: String,
    #[serde(rename = "redisUrl")]
    redis_url: Option<String>,
}

#[derive(Deserialize, Debug, Default)]
struct WarpMinioConfig {
    #[serde(rename = "serverPort")]
    server_port: Option<u16>,
    #[serde(rename = "disabledAuth")]
    disabled_auth: bool,
    #[serde(rename = "defaultRedisUrl")]
    default_redis_url: Option<String>,
    #[serde(rename = "matchPrefix")]
    match_prefix: Option<String>,
    #[serde(rename = "reParsingContentType")]
    re_parsing_content_type: bool,
    #[serde(rename = "power")]
    power: Option<HashMap<String, MinioConfig>>,
}
