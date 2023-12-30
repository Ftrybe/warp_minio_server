# 说明

warp_minio_server 主要用于转发minio请求。通过配置文件可以配置多个minio服务，通过不同的前缀来区分不同的minio服务。

# 安装

前往[Rust官网](https://www.rust-lang.org/)下载对应的安装包

### 运行
```shell
cargo run
```

### 编译
```shell
cargo build --release
```

### 部署
```shell
./target/release/warp_minio_server
```


### 启动服务

配置 WARP_MINIO_CONFIG 环境变量到 warp_minio_config.json文件
```shell
./warp_minio_server.exe
```

### nginx配置

```shell
	location ~ ^/minio/(.*) {
			proxy_pass http://127.0.0.1:9928/minio/$1$is_args$args;
			proxy_pass_request_headers on;
			#proxy_set_header Host $host;
			proxy_set_header X-Real-IP $remote_addr;
			proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
		}
```


### mac编译windows可执行文件

```shell
# 移除rust
brew remove rust
# 安装 rustup 工具链管理器，它可以帮助你管理和安装不同的 Rust 工具链
brew install rustup
#  x86_64-w64-mingw32-gcc
brew install mingw-w64

# 目标工具链，以便能够为 Windows 64位 系统编译 Rust 程序
rustup target add x86_64-pc-windows-gnu

# arm64 gnu 交叉编译
rustup target add aarch64-unknown-linux-gnu
# Windows 64位 系统生成二进制文件的交叉编译器
brew install llvm

cargo build --target x86_64-pc-windows-gnu --release
```

### MINIO配置参数



```yaml
server-port: 9928
match-prefix: /minio
parsing-content-type: false
auth-type: None
default:
  bucket-name: atom
  minio-config:
    access-key: accessKey
    secret-key: secretKey
    endpoint: http://127.0.0.1:9090
    max-pool-size: 20
    idle-pool-size: 5
  redis-config:
    host: http://127.0.0.1
    port: 6379
    db: 9
power:
  minio-atom:
    bucket-name: atom
    minio-config:
      - access-key: accessKey
        secret-key: secretKey
        endpoint: http://127.0.0.1:9090
        max-pool-size: 20
        idle-pool-size: 5
    redis-config:
      - host: http:127.0.0.1
        port: 6379
        db: 9
        password: ''
        max-pool-size: 20
        idle-pool-size: 5
    convert:
      accessKey: access-key
      secretKey: secret-key
      maxPoolSize: max-pool-size
      idlePoolSize: idle-pool-size
      bucketName: bucket-name
```
#### 容器配置

```yaml
server-port: 9928
match-prefix: /minio
parsing-content-type: false
auth-type: None
```
*   **server-port**: 当前服务启动端口。在此配置中，设置为 `9928`。
*   **match-prefix**: 用于匹配传入请求的 URL 路径前缀。这里设置为 `/minio`。
*   **parsing-content-type**: 是否根据后缀重新解析`content-type`,使用`mime_guess`解析。在此配置中设置为 `false`。
*   **auth-type**: 使用的认证类型。当前设置为 `None`，表示没有认证。
    * `None`表示没有认证，
    * `Bearer(key)`将获取Header中的Authorization字段,去除`Bearer `前缀后字符串从redis中查看是否存在数据验证权限。如：设置为`Bearer(SYS:USER:)`请求头`Authorization: Bearer 12333111`，将从redis中查看`SYS:USER:12333111`是否存在，存在则验证通过，否则验证失败。
    * `Basic(params_key,params_value)` 获取请求头中，key为`params_key`的值，验证是否和设置的params_value是否一致，一致则通过验证

#### 默认配置

```yaml
default:
  bucket-name: atom
  minio-config:
    access-key: accessKey
    secret-key: secretKey
    endpoint: http://127.0.0.1:9090
    max-pool-size: 20
    idle-pool-size: 5
  redis-config:
    host: http://127.0.0.1
    port: 6379
    db: 9
```
*   **bucket-name**: MinIO 桶的默认名称，设置为 `atom`。
*   **minio-config**: MinIO 的默认配置。
    *   **access-key**: MinIO 的访问密钥，此处为 `accessKey`。
    *   **secret-key**: MinIO 的密钥，此处为 `secretKey`。
    *   **endpoint**: MinIO 服务端点，`http://127.0.0.1:9090`。
    *   **max-pool-size**: 连接池的最大大小，此配置中为 `20`。
    *   **idle-pool-size**: 池中空闲连接的数量，设置为 `5`。
*   **redis-config**: Redis 的默认配置。
    *   **host**: Redis 服务器主机，`http://127.0.0.1`。
    *   **port**: Redis 服务器端口，`6379`。
    *   **db**: Redis 数据库索引，设置为 `9`。

##### 扩展配置
```yaml
power:
  minio-atom:
    bucket-name: atom
    minio-config:
      - access-key: accessKey
        secret-key: secretKey
        endpoint: http://127.0.0.1:9090
        max-pool-size: 20
        idle-pool-size: 5
    redis-config:
      - host: http:127.0.0.1
        port: 6379
        db: 9
        password: ''
        max-pool-size: 20
        idle-pool-size: 5
    convert:
      accessKey: access-key
      secretKey: secret-key
      maxPoolSize: max-pool-size
      idlePoolSize: idle-pool-size
      bucketName: bucket-name
```

**minio-atom**: 标记为 `minio-atom` 的特定配置集。

*   **bucket-name**: 此配置专用的 MinIO 桶名称，`atom`。
*   **minio-config**: `minio-atom` 的特定 MinIO 配置细节。
    *   类似于上面描述的默认 MinIO 配置。
*   **redis-config**: `minio-atom` 的特定 Redis 配置细节。
    *   类似于默认的 Redis 配置，但包括一个 `password` 字段。
*   **convert**: 映射字段以转换配置键。
    *   将特定字段映射到各自的配置键。