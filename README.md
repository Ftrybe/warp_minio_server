# 说明

warp_minio_server 是一个rust脚本，用于自签名minio附件。

> main.rs 主程序入口  
> Cargo.toml 依赖管理
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


### 启动编译文件

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

服务需要配合oss-config使用, 站点自定义域名`SITE_DOMAIN`, 例如与上述nginx匹配的配置为 `http://127.0.0.1:9090/prefix/config-key`

启动时设置环境变量WARP_MINIO_CONFIG_PATH可以自定义配置。


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
cargo build --target aarch64-unknown-linux-gnu --release
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
