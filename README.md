# S3 Client（S3/OSS 管理工具）

一个基于 Tauri + Vue 3 的桌面端 S3/OSS 管理工具，支持多配置管理、对象浏览、上传/下载、预览与批量删除，适配 S3 兼容协议（如 AWS S3、MinIO、阿里云 OSS 等）。

## 主要特性

- 多配置管理：新增、编辑、删除、连接测试
- 对象浏览：目录化列表、分页加载、面包屑导航
- 文件操作：上传文件/文件夹、下载、删除（含递归删除目录）
- 预览能力：图片/视频/文本预览，其他类型提示下载
- 传输进度：上传/下载实时进度、任务面板
- 拖拽上传：支持拖拽文件/文件夹到窗口上传
- S3 兼容：支持自定义 `endpoint`、`region`、`bucket`、访问风格

## 技术栈

- 前端：Vue 3、TypeScript、Vite、Ant Design Vue
- 桌面端：Tauri v2
- 后端（Rust）：AWS SDK for Rust（S3）

## 快速开始

### 1. 安装依赖

```bash
yarn install
```

### 2. 启动开发

```bash
yarn tauri dev
```

说明：`src-tauri/tauri.conf.json` 中配置了 `beforeDevCommand: yarn dev`，因此 Tauri 会自动拉起前端开发服务。

### 3. 构建打包

```bash
yarn tauri build
```

## 使用说明

### 配置管理

进入应用后默认是“配置列表”页，可进行：

- 新增配置：填写连接信息并保存
- 编辑配置：更新已有连接信息
- 测试连接：后端会进行一次 `list_objects` 测试
- 删除配置：从本地配置文件移除

### 配置字段说明

- `name`：连接名称（显示用）
- `provider`：存储提供商（示例：`aws`、`minio`、`aliyun`）
- `region`：区域（默认 `cn-north-1`，可自定义）
- `accessKey` / `secretKey`：访问密钥
- `endpoint`：S3 兼容端点地址（如 MinIO/OSS 的地址）
- `bucket`：存储桶名称
- `pathStyle`：访问样式。`path` 表示路径样式（`https://endpoint/bucket/key`），`virtual` 表示虚拟托管样式（`https://bucket.endpoint/key`）

### 文件管理

进入某个配置后可进行对象操作：

- 浏览目录：双击目录进入，面包屑返回
- 上传文件/文件夹：选择后加入上传队列
- 下载：选择保存目录后后台下载
- 删除：支持递归删除目录
- 预览：图片 / 视频使用预签名 URL；文本文件下载后本地解码展示

### 传输与进度

- 上传/下载会加入队列，右下角显示进度摘要
- 可展开查看详细任务列表与状态
- 后端并发数限制为 `5`（见 `MAX_CONCURRENT`）

## 数据与配置存储

配置文件由后端写入 Tauri 的 `app_config_dir`，文件名为 `config.json`。配置采用 JSON 格式，字段为驼峰命名。

## 后端命令（Tauri Invoke）

前端通过 `@tauri-apps/api` 调用后端命令：

- 配置管理：`config_get`、`config_save`、`config_delete`、`config_test`
- 文件管理：`file_list`、`file_download`、`file_delete`、`file_get_preview_url`、`file_upload`、`file_download_path`

## 目录结构

```text
src/
  views/
    ConfigList.vue        # 配置列表
    ConfigForm.vue        # 新增/编辑配置
    FileManager.vue       # 文件管理页
    components/
      TransferIndicator.vue # 传输进度
      FilePreviewModal.vue  # 文件预览
      ContextMenu.vue       # 右键菜单
  services/
    config.ts             # 配置 API
    file.ts               # 文件 API
  utils/
    utils.ts              # 文件类型/格式化
src-tauri/
  src/
    commands/             # Tauri 命令
    utils/oss.rs          # S3 访问封装
    config.rs             # 配置读写
```

## 备注

- 上传大文件时会自动使用分片上传（默认分片大小 5MB）。
- 下载大文件采用分片拉取（默认分片大小 2MB）。
- 文件列表支持分页加载（每次最多 1000 条）。

如果你希望增加新云厂商、批量管理能力或更细粒度权限控制，欢迎继续扩展该项目。
