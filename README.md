# ACE Limiter

基于 Next.js + Tauri 构建的桌面应用。

前端不开源，后端代码开源，您可自由审查代码是否安全。

专门处理 某讯 的 ACE 程序...

## 界面样式

### 浅色模式
<img width="386" height="730" alt="image" src="https://github.com/user-attachments/assets/8d7000cd-919b-4f98-a227-65261d83405c" />
<img width="386" height="730" alt="image" src="https://github.com/user-attachments/assets/ddb1910b-aa64-47e6-908c-e6e10484439d" />

### 深色模式
<img width="386" height="730" alt="image" src="https://github.com/user-attachments/assets/cd7f5943-e1bb-4790-80d8-68d46d44a469" />
<img width="386" height="730" alt="image" src="https://github.com/user-attachments/assets/2a44908c-868b-4478-b78c-c400b44d95a4" />
<img width="386" height="730" alt="image" src="https://github.com/user-attachments/assets/64fe74b9-141a-4ba7-85e2-1eb8c8ef309e" />


## 环境要求

在打包之前，请确保你的系统已安装以下工具：

- [Node.js](https://nodejs.org/) (推荐 v18+)
- [Rust](https://www.rust-lang.org/tools/install)
- [Tauri CLI](https://tauri.app/)

## 安装依赖

```bash
# 安装 Tauri CLI (首次使用需要)
cargo install tauri-cli
```

## 打包构建

### 手动执行

```bash
# 1. 构建 Tauri 应用
cd src-tauri
cargo tauri build
```

## 构建产物

打包完成后，安装包位于：

```
src-tauri/target/release/bundle/
├── msi/          # Windows MSI 安装包
└── nsis/         # Windows NSIS 安装包
```

## 常见问题

### `cargo tauri` 命令不存在

需要先安装 Tauri CLI：

```bash
cargo install tauri-cli
```

### Rust 编译缓慢

首次编译会下载和编译依赖，耗时较长，后续构建会快很多。
