# ACE Limiter

基于 Next.js + Tauri 构建的桌面应用。

前端不开源，后端代码开源，您可自由审查代码是否安全。

专门处理 某讯 的 ACE 程序...

## 界面样式

<img width="367" height="754" alt="image" src="https://github.com/user-attachments/assets/c23c8e09-321c-4a02-be62-4487390d3296" />
<img width="367" height="754" alt="image" src="https://github.com/user-attachments/assets/8bdb944d-60bd-4681-b754-e4630d60f6d0" />


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
