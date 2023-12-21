
use bytes::Bytes;

use reqwest::StatusCode;
use warp::Rejection;
use crate::config;
use crate::cache;



pub fn auth_header_bearer(auth_header: Option<String> ,config_key: &str) -> bool {
    if config::WARP_MINIO_CONFIG.disabled_auth {
        return true;
    }

    let auth_str = match auth_header {
        Some(auth) => auth,
        None => return false,
    };

    if !auth_str.starts_with("Bearer ") {
        return false;
    }

    let token = match trim_bearer_prefix(&auth_str) {
        Some(token) => token,
        None => return false,
    };

    match cache::fetch_user_info_from_redis(config_key, token) {
        Ok(Some(_)) => true,
        _ => false,
    }
}


pub fn unauthorized_reply() -> Result<Box<dyn warp::Reply>, Rejection> {
    let body = Bytes::from_static(b"{\"error\": \"Unauthorized\"}");
    let response = warp::http::Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .body(body)
        .unwrap();
    Ok(Box::new(response) as Box<dyn warp::Reply>)
}



fn trim_bearer_prefix(auth_header: &str) -> Option<&str> {
    if auth_header.len() < config::BEARER_PREFIX_LEN || &auth_header[..config::BEARER_PREFIX_LEN] != "Bearer " {
        None
    } else {
        Some(&auth_header[config::BEARER_PREFIX_LEN..])
    }
}
