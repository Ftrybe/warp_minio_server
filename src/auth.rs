use std::fmt;

use bytes::Bytes;
use r2d2_redis::redis::{Commands, RedisResult};
use reqwest::StatusCode;
use serde::de::{MapAccess, Visitor};
use serde::{de, Deserialize, Deserializer};
use warp::http::HeaderMap;
use warp::Rejection;

use crate::cache;
use crate::config;

pub fn check(auth_header: HeaderMap, config_key: &str) -> bool {
    match &config::WARP_MINIO_CONFIG.auth_type {
        None => true,
        Some(auth) => {
            match auth {
                AuthType::Bearer(redis_key) => auth_header_bearer(auth_header, &redis_key, config_key),
                AuthType::Basic(params_key, params_value) => {
                    // 确保这里是引用或复制，以避免移动
                    auth_header_basic(auth_header, params_key, params_value)
                },
                AuthType::None => true
            }
        }
    }
}

fn auth_header_basic(headers: HeaderMap, params_key: &String, params_value: &String) -> bool {
   // 如果config_key不为空
    if let Some(header_params_value) = headers.get(params_key) {
        return header_params_value.eq(params_value)
    }
    false
}

fn auth_header_bearer(headers: HeaderMap, redis_key: &str ,config_key: &str) -> bool {
    let auth_header = headers.get("authorization");

    let auth_str = match auth_header {
        Some(auth) => auth,
        None => return false,
    };

    if !auth_str.to_str().unwrap_or("").starts_with("Bearer ") {
        return false;
    }

    let token = match trim_bearer_prefix(auth_str.to_str().unwrap_or("")) {
        Some(token) => token,
        None => return false,
    };

    match fetch_user_info_from_redis( config_key,redis_key, token) {
        Ok(Some(_)) => true,
        _ => false,
    }
}
// 根据token 获取redis中的用户信息
pub fn fetch_user_info_from_redis(config_key: &str,redis_key: &str, token: &str) -> RedisResult<Option<String>> {

    // 从连接池获取连接
    let pool = match cache::get_redis_pool(config_key) {
        Ok(pool) => pool,
        Err(_) => return Ok(None)
    };

    let mut con = pool
        .get()
        .expect("Failed to get connection from pool");

    // redis_key => "Authorization:login:token:{}"
    let user_token_key = format!("{}{}", redis_key, token);
    // 使用连接执行Redis GET命令
    con.get(&user_token_key)
}



pub fn error_reply(error_type: ErrorReply) -> Result<Box<dyn warp::Reply>, Rejection> {
    let (body, status_code) = match error_type {
        ErrorReply::Unauthorized => {
            let body = Bytes::from_static(b"{\"error\": \"Unauthorized\"}");
            (body, StatusCode::UNAUTHORIZED)
        }
        ErrorReply::MinioInvalid => {
            let body = Bytes::from_static(b"{\"error\": \"Config error\"}");
            (body, StatusCode::INTERNAL_SERVER_ERROR)
        }
    };
    let response = warp::http::Response::builder()
        .status(status_code)
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


pub enum ErrorReply {
    Unauthorized,
    MinioInvalid,
}

#[derive(Debug)]
pub enum AuthType {
    // redis_key
    Bearer(String),
    // params_key, params_value
    Basic(String, String),
    None,
}

impl fmt::Display for AuthType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthType::Bearer(_) => write!(f, "Bearer"),
            AuthType::Basic(user, pass) => write!(f, "Basic: {}, {}", user, pass),
            AuthType::None => write!(f, "None"),
        }
    }
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "lowercase")]
enum Field { Bearer, Basic, None }

// 自定义Visitor来处理AuthType的解析
struct AuthTypeVisitor;

impl<'de> Visitor<'de> for AuthTypeVisitor {
    type Value = AuthType;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an object with keys `Bearer`, `Basic` or a `None` value")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
    {
        match value {
            "None" => Ok(AuthType::None),
            _ => Err(de::Error::unknown_variant(value, &["Bearer", "Basic", "None"])),
        }
    }

    fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
        where
            V: MapAccess<'de>,
    {
        let mut bearer = None;
        let mut basic = None;
        while let Some(key) = map.next_key()? {
            match key {
                Field::Bearer => {
                    if bearer.is_some() {
                        return Err(de::Error::duplicate_field("Bearer"));
                    }
                    bearer = Some(map.next_value()?);
                },
                Field::Basic => {
                    if basic.is_some() {
                        return Err(de::Error::duplicate_field("Basic"));
                    }
                    let values: (String, String) = map.next_value()?;
                    basic = Some(values);
                },
                Field::None => {
                    return Err(de::Error::custom("`None` should not be a map key"));
                }
            }
        }
        let result = match (bearer, basic) {
            (Some(token), None) => AuthType::Bearer(token),
            (None, Some((key, value))) => AuthType::Basic(key, value),
            (None, None) => AuthType::None,
            _ => return Err(de::Error::custom("Expected either `Bearer` or `Basic`"))
        };
        Ok(result)
    }
}

// 为AuthType实现Deserialize trait
impl<'de> Deserialize<'de> for AuthType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
    {
        deserializer.deserialize_any(AuthTypeVisitor)
    }
}