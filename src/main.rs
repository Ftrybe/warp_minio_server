
mod config;
mod auth;
mod cache;
mod minio_parser;
mod r2d2_minio;

use lazy_static::lazy_static;

use minio::s3::client::Client;

use std::collections::HashMap;
use std::{env};
use std::string::String;
use std::sync::{Arc, Mutex};

use warp::{Filter, Rejection};
use auth::unauthorized_reply;
use mime_guess::from_path;
use warp::http::HeaderMap;


// 全局静态变量连接池
lazy_static! {
    static ref CLIENT: reqwest::Client = reqwest::Client::builder()
        .build()
        .unwrap();

    static ref MINIO_POOLS: Arc<Mutex<HashMap<String, Client>>> = {
        Arc::new(Mutex::new(HashMap::new()))
    };
}

#[tokio::main]
async fn main() {
    env::set_var("RUST_LOG", "info");
    env_logger::init();
    cache::initialize_redis_pools();
    minio_parser::initialize_minio_pools();

    let cors = warp::cors()
        .allow_any_origin();

    let optional_auth_header = warp::header::optional("authorization");

    let route = warp::path::full()
        .and(warp::query::<HashMap<String, String>>())
        .and(optional_auth_header)
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
        "Auth is disabled: {}",
        config::WARP_MINIO_CONFIG.disabled_auth.to_string()
    );

    warp::serve(route).run(([127, 0, 0, 1], server_port)).await;
}
async fn process(
    path: warp::path::FullPath,
    params: HashMap<String, String>,
    header: Option<String>,
    _method: warp::http::Method,
    headers: warp::http::HeaderMap,
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
    let key = parts.next().unwrap_or("");
    let filename = params.get("filename");

    log::info!("Access: {}", request_uri);

    if !auth::auth_header_bearer(header, config_key) {
        return unauthorized_reply();
    }

    let link = match minio_parser::get_generate_link_by_config_key_and_object_key(config_key, key).await {
        Ok(token) => token,
        Err(e) => {
            log::error!("Failed to generate link: {}", e);
            return unauthorized_reply()
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
    if config::WARP_MINIO_CONFIG.re_parsing_content_type {
        let content_type = re_parse_content_type(&headers, key);
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


fn re_parse_content_type(headers: &&HeaderMap, key: &str ) -> String {
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


