# ncmpp-rust

一个极速的多线程 ncm 解密器，使用 Rust 重写。

## 使用方法

### 快速开始

下载 Release 中的 `ncmpp.exe`，放到含有 `.ncm` 文件的目录中，双击运行。

解密后的文件存放在 `unlock` 文件夹中，格式自动识别（mp3 / flac 等）。

### 命令行参数

```
ncmpp [OPTIONS]
```

| 参数 | 简写 | 说明 |
|---|---|---|
| `--threads <N>` | `-t <N>` | 最大解密线程数（默认：CPU 逻辑核心数） |
| `--showtime` | `-s` | 显示解密消耗的时间 |

示例：

```
ncmpp                          # 使用全部线程
ncmpp -t 4                     # 使用 4 个线程
ncmpp -t 4 -s                  # 4 线程并显示用时
```

## 构建

需要 [Rust](https://www.rust-lang.org/) 1.63+。

```sh
cargo build --release
```

输出文件：`target/release/ncmpp`（Linux/macOS）或 `target/release/ncmpp.exe`（Windows）。

无需 Visual Studio、CMake 或 OpenSSL，`cargo` 会自动处理所有依赖。

## 技术栈

- **CLI**: [clap](https://crates.io/crates/clap)
- **AES-128-ECB**: [aes](https://crates.io/crates/aes)
- **Base64**: [base64](https://crates.io/crates/base64)
- **JSON**: [serde_json](https://crates.io/crates/serde_json)

## 许可

MIT
