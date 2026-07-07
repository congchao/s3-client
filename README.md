# S3 Client

S3 Client 是一个基于 Tauri 2、Vue 3 和 Rust 的桌面端 S3/OSS/MinIO 管理工具。它面向 S3 兼容对象存储，提供多账号配置、桶浏览、对象管理、异步传输队列和文件预览等能力。

## 主要功能

- 多配置管理：新增、编辑、复制、删除、连接测试。
- 多桶管理：配置可不绑定默认桶，支持直接列出账号下所有 bucket。
- 对象浏览：目录化列表、分页加载、面包屑导航、当前目录搜索。
- 文件操作：新建文件夹、上传文件、上传文件夹、下载、删除、递归删除目录。
- 对象管理：重命名、移动、复制文件或目录。
- 链接分享：生成自定义有效期的预签名下载链接。
- 传输队列：上传/下载异步队列、进度展示、取消任务、失败重试。
- 权限探测：进入 bucket 后探测列出、读取、写入、删除权限，并按权限限制操作。
- 文件预览：
  - 图片、视频预览。
  - 文本、JSON、Markdown 预览。
  - CSV/TSV 表格预览。
  - Parquet 表格预览、Schema 查看、分页浏览。
- 安全存储：配置存储在 SQLite，`secretKey` 使用 AES-256-GCM 加密。

## 技术栈

- 桌面端：Tauri v2
- 前端：Vue 3、TypeScript、Vite、Ant Design Vue
- 后端：Rust、AWS SDK for Rust
- 本地存储：SQLite、AES-256-GCM
- 表格预览：Apache Arrow、parquet-wasm

## 环境要求

- Node.js
- Yarn
- Rust stable
- Tauri v2 所需系统依赖

macOS 构建需要 Xcode Command Line Tools。Windows 构建需要 Microsoft C++ Build Tools、Windows SDK 和 WebView2 相关环境。

## 安装依赖

```bash
yarn install
```

## 开发运行

```bash
yarn tauri dev
```

`src-tauri/tauri.conf.json` 已配置 `beforeDevCommand: yarn dev`，运行 Tauri 开发命令时会自动启动前端 Vite 服务。

## 前端构建检查

```bash
yarn build
```

该命令执行 TypeScript 检查并构建前端静态资源。

## 桌面端打包

通用打包命令：

```bash
yarn tauri build
```

项目也提供了按平台区分的脚本。

macOS Apple Silicon：

```bash
yarn package:mac:arm64
```

macOS Intel：

```bash
yarn package:mac:x64
```

Windows amd64：

```powershell
yarn package:win:amd64
```

产物目录：

```text
src-tauri/target/aarch64-apple-darwin/release/bundle
src-tauri/target/x86_64-apple-darwin/release/bundle
src-tauri\target\x86_64-pc-windows-msvc\release\bundle
```

Windows 包建议在 Windows 机器或 Windows CI Runner 上构建，不建议从 macOS 直接交叉打包 Windows 安装包。

更详细的脚本说明见 [scripts/README.md](scripts/README.md)。

## 应用更新

应用启动时会检查 [congchao/s3-client](https://github.com/congchao/s3-client) 的最新 GitHub Release，应用菜单中的“检查更新”也可以手动触发检查。检查到的新版本会保存到本地 SQLite；用户选择“跳过此版本”后，同一版本在后续启动检查中不会再自动提示。

实际下载和替换安装使用 Tauri updater。发布 Release 时需要上传 Tauri 生成的签名更新包和 `latest.json`，并在构建应用时配置匹配的 updater 公钥，否则用户点击升级时会提示更新清单或签名校验失败。

## 配置说明

新增配置时需要填写：

- `name`：连接名称。
- `provider`：存储提供商，例如 `aws`、`minio`、`aliyun`。
- `region`：区域，例如 `us-east-1`、`cn-north-1`。
- `accessKey`：访问密钥 ID。
- `secretKey`：访问密钥 Secret。
- `endpoint`：S3 兼容端点地址。
- `bucket`：默认存储桶，可选。不填写时进入配置会展示该账号下所有 bucket。
- `pathStyle`：访问样式，`path` 表示路径样式，`virtual` 表示虚拟托管样式。

MinIO 超管账号可以不填写默认 bucket，进入后通过 bucket 列表选择要管理的桶。

## 数据存储

本地配置存储在 Tauri 的 `app_config_dir` 下：

- `config.sqlite`：SQLite 配置数据库。
- `secret.key`：本机 AES 加密密钥。

`secretKey` 不以明文存储在数据库中，而是使用 `secret.key` 通过 AES-256-GCM 加密后保存。

macOS 默认目录通常类似：

```text
~/Library/Application Support/com.mironizz.s3-client/
```

不同系统的实际目录由 Tauri `app_config_dir` 决定。

## 使用说明

### 配置列表

应用启动后进入配置列表，可新增、编辑、复制、测试和删除配置。点击“进入”或双击配置：

- 如果配置有默认 bucket，则直接进入文件管理页。
- 如果配置没有默认 bucket，则进入 bucket 列表页。

### Bucket 列表

Bucket 列表会展示当前账号可访问的 bucket。双击或点击“进入”可打开对应 bucket 的文件管理页。

### 文件管理

文件管理页支持：

- 双击目录进入。
- 面包屑返回上级目录。
- 上传文件或文件夹。
- 新建文件夹。
- 下载文件或目录。
- 删除文件或递归删除目录。
- 右键复制路径、重命名、移动、复制对象、生成预签名链接。
- 刷新时可取消 loading 等待，避免后端请求异常导致界面一直阻塞。

### 传输队列

右下角显示传输摘要。展开后可查看上传和下载任务：

- 等待中、上传中、下载中任务可以取消。
- 失败任务可以重试。
- 队列最大等待任务数为 `1000`。
- 后端默认最大并发数为 `5`。

### 文件预览

支持的预览类型：

- 图片、视频：通过预签名 URL 预览。
- 文本：本地解码展示。
- JSON：格式化展示。
- Markdown：渲染预览。
- CSV/TSV：表格预览。
- Parquet：10MB 以内支持表格预览、Schema 查看和分页浏览。超过 10MB 时只提供下载按钮。

## 后端命令概览

前端通过 Tauri invoke 调用后端命令。

配置管理：

- `config_get`
- `config_save`
- `config_delete`
- `config_test`

Bucket：

- `bucket_list`
- `bucket_probe_permissions`

文件管理：

- `file_list`
- `file_download`
- `file_delete`
- `file_get_preview_url`
- `file_create_presigned_url`
- `file_create_directory`
- `file_copy`
- `file_move`
- `file_upload`
- `file_download_path`

传输队列：

- `file_transfer_cancel`
- `file_transfer_retry`

## 目录结构

```text
src/
  router/                 # 前端路由
  services/               # Tauri invoke API 封装
  types/                  # 类型定义和常量
  utils/                  # 前端工具函数
  views/
    ConfigList.vue        # 配置列表
    ConfigForm.vue        # 新增/编辑配置
    BucketList.vue        # Bucket 列表
    FileManager.vue       # 文件管理
    components/
      ContextMenu.vue       # 右键菜单
      FilePreviewModal.vue  # 文件预览
      TransferIndicator.vue # 传输队列
src-tauri/
  src/
    commands/             # Tauri 命令
    config.rs             # SQLite 配置和密钥加密
    models/               # Rust 数据模型
    utils/oss.rs          # S3/OSS 操作封装
scripts/                  # 打包脚本
```

## 注意事项

- Windows 安装包请在 Windows 环境构建。
- 未配置默认 bucket 的账号需要拥有 `ListBuckets` 权限，否则无法列出 bucket。
- 权限探测基于实际 S3 操作结果，仅用于前端操作提示和基础拦截。
- S3 目录是对象 key 前缀模拟出来的，不是真实文件系统目录。
- 目录移动和复制会遍历前缀下所有对象，大目录操作可能耗时较长。
