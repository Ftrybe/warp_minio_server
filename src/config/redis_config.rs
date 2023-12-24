use serde::Deserialize;

#[derive(Deserialize, Debug, Default, Clone)]
pub struct RedisConfig {
    #[serde(rename = "host")]
    pub(crate) host: String,
    #[serde(rename = "port")]
    pub(crate) port: Option<u16>,
    #[serde(rename = "db")]
    pub(crate) db: Option<u8>,
    #[serde(rename = "username")]
    pub(crate) username: Option<String> ,
    #[serde(rename = "password")]
    pub(crate) password: Option<String>,
    #[serde(rename = "max-pool-idle")]
    pub(crate) max_pool_size: Option<u32>,
    #[serde(rename = "idle-pool-size")]
    pub(crate) idle_pool_size: Option<u32>,
}

impl RedisConfig {
    pub fn redis_url(&self) -> String {
        let mut url = String::from("redis://");

        // 如果提供了用户名和密码，加入到 URL 中
        if self.username.is_some() || self.password.is_some() {
            url.push_str(&format!(
                "{}:{}@",
                self.username.as_deref().unwrap_or(""),
                self.password.as_deref().unwrap_or("")
            ));
        }

        // 添加主机名
        url.push_str(&self.host);

        // 如果提供了端口，加入到 URL 中
        if let Some(port) = self.port {
            url.push_str(&format!(":{}", port));
        }

        // 如果提供了数据库索引，加入到 URL 中
        if let Some(db) = self.db {
            url.push_str(&format!("/{}", db));
        }

        url
    }

    pub fn pool_key(&self) -> String {
        format!("{}:{:?}", self.host, self.port)
    }
}
