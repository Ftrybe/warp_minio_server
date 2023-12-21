
mod config;
mod auth;
mod cache;
mod minio_parser;

use lazy_static::lazy_static;

use minio::s3::client::Client;

use std::collections::HashMap;
use std::{env};
use std::string::String;
use std::sync::{Arc, Mutex};

use warp::{Filter, Rejection};
use auth::unauthorized_reply;
use mime_guess::from_path;



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
    _headers: warp::http::HeaderMap,
    _body: bytes::Bytes,
) -> Result<Box<dyn warp::Reply>, Rejection> {
    // 在这里处理你的业务逻辑，例如与数据库交互，文件操作等
    let request_uri = path.as_str();

    let mut url_prefix = config::URL_PREFIX; // 用您的默认前缀替换此处

    // 检查 WARP_MINIO_CONFIG.match_prefix 是否不为空
    if let Some(ref prefix) = config::WARP_MINIO_CONFIG.match_prefix {
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

    if !auth::auth_header_bearer(header, config_key) {
        return unauthorized_reply();
    }

    let link = match minio_parser::get_generate_link_by_config_key_and_object_key(config_key, key).await {
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
            if config::WARP_MINIO_CONFIG.re_parsing_content_type {
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



