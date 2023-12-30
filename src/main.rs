use std::collections::HashMap;
use std::env;
use std::string::String;

use lazy_static::lazy_static;
use mime_guess::from_path;
use warp::{Filter, Rejection};
use warp::http::HeaderMap;

use crate::auth::{error_reply, ErrorReply};
use crate::minio::minio_pool::{MINIO_POOLS, MinioPool};

mod config;
mod auth;
mod cache;
mod minio;

// 全局静态变量连接池
lazy_static! {
    static ref CLIENT: reqwest::Client = reqwest::Client::builder()
        .build()
        .unwrap();
}

#[tokio::main]
async fn main() {
    env::set_var("RUST_LOG", "info");
    env_logger::init();
    cache::initialize_redis_pools();
    minio::minio_pool::initialize_minio_pools().await;

    let cors = warp::cors()
        .allow_any_origin();

    let route = warp::path::full()
        .and(warp::query::<HashMap<String, String>>())
        .and(warp::method())
        .and(warp::header::headers_cloned())
        .and_then(process)
        .with(cors);

    let mut server_port = config::PORT; // 用您的默认前缀替换此处

    if let Some(ref prefix) = config::WARP_MINIO_CONFIG.server_port {
        if *prefix != 0 {
            // 假设0是一个无效的端口号
            server_port = *prefix;
        }
    }

    log::info!(
    "Auth type: {}",
    config::WARP_MINIO_CONFIG.auth_type
        .as_ref()
        .map_or("None".to_string(), |auth_type| auth_type.to_string())
    );

    // 启动健康检查任务
    tokio::spawn(async move {
        MinioPool::perform_health_checks().await;
    });


    warp::serve(route).run(([127, 0, 0, 1], server_port)).await;
}

async fn process(
    path: warp::path::FullPath,
    params: HashMap<String, String>,
    _method: warp::http::Method,
    headers: HeaderMap,
) -> Result<Box<dyn warp::Reply>, Rejection> {
    let request_uri = path.as_str();

    let url_prefix = config::WARP_MINIO_CONFIG
        .match_prefix
        .as_deref()
        .unwrap_or(config::URL_PREFIX);

    if !request_uri.starts_with(url_prefix) {
        return Ok(Box::new(warp::reply::with_status(
            "URI does not start with the expected prefix",
            warp::http::StatusCode::BAD_REQUEST,
        )));
    }

    let bucket_path = &request_uri[url_prefix.len()..];
    let path = bucket_path.trim_start_matches('/');
    let mut parts = path.splitn(2, '/');
    let config_key = parts.next().unwrap_or("");
    let object_key = parts.next().unwrap_or("");
    let filename = params.get("filename");

    log::info!("Access: {}", request_uri);

    if !auth::check(headers.clone(), config_key) {
        return error_reply(ErrorReply::Unauthorized);
    }

    let link = match minio::minio_parser::get_generate_link_by_config_key_and_object_key(config_key, object_key).await {
        Ok(token) => token,
        Err(e) => {
            log::error!("Failed to generate link: {}", e);
            return error_reply(ErrorReply::MinioInvalid);
        }
    };

    let client_request = CLIENT.get(&link);
    let client_request = if let Some(range_header) = headers.get("Range") {
        client_request.header("Range", range_header)
    } else {
        client_request
    };

    // 发送请求并获取异步的响应流
    let response = client_request.send().await.map_err(|_| warp::reject::reject())?;
    let status = response.status();
    // let headers = response.headers().clone();
    let headers = &response.headers().clone();
    // 使用 `hyper::Body::wrap_stream` 将响应流转换为 warp 可以发送的 Body
    let stream = response.bytes_stream();
    let body = warp::hyper::Body::wrap_stream(stream);

    let mut response_builder = warp::http::Response::builder().status(status);

    for (key, value) in headers {
        response_builder = response_builder.header(key, value);
    }

    // 如果设置了重新解析 Content-Type
    if config::WARP_MINIO_CONFIG.parsing_content_type {
        let content_type = re_parse_content_type(&headers, object_key);
        response_builder = response_builder.header("Content-Type", content_type);
    }

    if let Some(filename) = filename {
        let content_disposition = format!("attachment; filename=\"{}\"", filename);
        response_builder = response_builder.header("Content-Disposition", content_disposition);
    }

    let response = response_builder
        .body(body)
        .map_err(|_| warp::reject::reject())?;

    Ok(Box::new(response) as Box<dyn warp::Reply>)
}


fn re_parse_content_type(headers: &&HeaderMap, key: &str) -> String {
    headers.get("Content-Type")
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
        })
}


