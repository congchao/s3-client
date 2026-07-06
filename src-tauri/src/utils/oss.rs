use aws_config::timeout::TimeoutConfig;
use aws_config::Region;
use aws_sdk_s3::config::http::HttpResponse;
use aws_sdk_s3::config::{Credentials, SharedCredentialsProvider};
use aws_sdk_s3::error::SdkError;
use aws_sdk_s3::operation::delete_object::*;
use aws_sdk_s3::operation::get_object::*;
use aws_sdk_s3::operation::list_objects_v2::*;
use aws_sdk_s3::operation::put_object::*;
// 添加预签名相关导入
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::types::{Delete, ObjectIdentifier};
use aws_sdk_s3::Client;
use aws_smithy_types::byte_stream::ByteStream;

use crate::config::APP_CONFIG;
use crate::models::OssConfig;
use aws_smithy_types::checksum_config::RequestChecksumCalculation;
use aws_smithy_types::error::metadata::ProvideErrorMetadata;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;
use tokio::fs::File as AsyncFile;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

static OSS_CLIENT_CACHE: LazyLock<Mutex<HashMap<String, Arc<Oss>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub name: String,
    pub is_dir: bool,
    pub size: Option<i64>,
    pub last_modified: Option<String>,
    pub content_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileList {
    pub next_token: Option<String>,
    pub objects: Vec<FileInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BucketInfo {
    pub name: String,
    pub creation_date: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BucketPermissions {
    pub list: bool,
    pub read: bool,
    pub write: bool,
    pub delete: bool,
}

pub struct Oss {
    pub client: Client,
}

struct ObjectKeyPage {
    objects: Vec<String>,
    next_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectType {
    Directory,
    Object,
    NotFound,
}
impl Oss {
    // 从配置创建实例的方法
    pub fn new_with_config(oss_config: &OssConfig) -> Result<Self, String> {
        // 验证必要字段
        if oss_config.access_key.is_empty() || oss_config.secret_key.is_empty() {
            return Err("Access Key 或 Secret Key 不能为空".to_string());
        }

        if oss_config.endpoint.is_empty() {
            return Err("Endpoint 不能为空".to_string());
        }

        let aws_config = aws_sdk_s3::config::Builder::new()
            .behavior_version_latest()
            .region(Region::new(oss_config.region.clone()))
            .credentials_provider(SharedCredentialsProvider::new(Credentials::new(
                &oss_config.access_key,
                &oss_config.secret_key,
                None,
                None,
                "custom",
            )))
            .endpoint_url(&oss_config.endpoint)
            .timeout_config(
                TimeoutConfig::builder()
                    .operation_timeout(Duration::from_secs(30))
                    .connect_timeout(Duration::from_secs(10))
                    .build(),
            )
            .request_checksum_calculation(RequestChecksumCalculation::WhenRequired)
            .force_path_style(oss_config.path_style == "path")
            .build();

        Ok(Self {
            client: Client::from_conf(aws_config),
        })
    }

    pub fn new(id: &str) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let cnf = APP_CONFIG.lock().map_err(|e| e.to_string())?;
        let oss_config = cnf.get(id).cloned()?;
        Ok(Self::new_with_config(&oss_config)?)
    }

    pub fn new_cached(id: &str) -> Result<Arc<Self>, Box<dyn Error + Send + Sync>> {
        {
            let cache = OSS_CLIENT_CACHE.lock().map_err(|e| e.to_string())?;
            if let Some(oss) = cache.get(id) {
                return Ok(oss.clone());
            }
        }

        let oss = Arc::new(Self::new(id)?);
        let mut cache = OSS_CLIENT_CACHE.lock().map_err(|e| e.to_string())?;
        if let Some(cached) = cache.get(id) {
            return Ok(cached.clone());
        }
        cache.insert(id.to_string(), oss.clone());
        Ok(oss)
    }

    pub fn clear_cached_config(id: &str) {
        if let Ok(mut cache) = OSS_CLIENT_CACHE.lock() {
            cache.remove(id);
        }
    }

    pub async fn get_object(
        &self,
        bucket: &str,
        key: &str,
    ) -> Result<GetObjectOutput, SdkError<GetObjectError, HttpResponse>> {
        self.client
            .get_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
    }

    /**
     * 上传文件
     * @param key 文件名
     * @param path 文件路径
     */
    pub async fn put_object(
        &self,
        bucket: &str,
        key: &str,
        path: &str,
    ) -> Result<PutObjectOutput, Box<dyn Error + Send + Sync>> {
        let byte_stream = ByteStream::from_path(path)
            .await
            .map_err(|e| format!("创建字节流失败: {}，路径: {}", e, path))?;
        let content_length = tokio::fs::metadata(path).await?.len() as i64;
        // 根据文件路径推断 content-type
        let mime_type = mime_guess::from_path(path).first_or_octet_stream();
        let content_type = mime_type.essence_str().to_string();
        Ok(self
            .client
            .put_object()
            .bucket(bucket)
            .key(key)
            .body(byte_stream)
            .content_type(content_type)
            .content_length(content_length)
            .send()
            .await?)
    }

    pub async fn create_directory(
        &self,
        bucket: &str,
        directory_path: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let directory_key = Self::normalize_directory_prefix(directory_path);
        if directory_key.is_empty() {
            return Err("文件夹名称不能为空".into());
        }

        self.client
            .put_object()
            .bucket(bucket)
            .key(directory_key)
            .body(ByteStream::from_static(b""))
            .content_length(0)
            .send()
            .await?;

        Ok(())
    }

    async fn _delete_object(
        &self,
        bucket: &str,
        key: &str,
    ) -> Result<DeleteObjectOutput, SdkError<DeleteObjectError, HttpResponse>> {
        self.client
            .delete_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
    }

    fn chunk_delete_keys(keys: &[String]) -> Vec<Vec<String>> {
        keys.chunks(1000).map(|chunk| chunk.to_vec()).collect()
    }

    async fn delete_objects_batch(
        &self,
        bucket: &str,
        keys: &[String],
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        for chunk in Self::chunk_delete_keys(keys) {
            let objects = chunk
                .into_iter()
                .map(|key| ObjectIdentifier::builder().key(key).build())
                .collect::<Result<Vec<_>, _>>()?;

            self.client
                .delete_objects()
                .bucket(bucket)
                .delete(Delete::builder().set_objects(Some(objects)).build()?)
                .send()
                .await?;
        }

        Ok(())
    }

    fn normalize_directory_prefix(directory_path: &str) -> String {
        let directory_path = directory_path.trim_matches('/');
        if directory_path.is_empty() {
            String::new()
        } else {
            format!("{}/", directory_path)
        }
    }

    fn encode_copy_source(bucket: &str, key: &str) -> String {
        fn encode_segment(input: &str) -> String {
            input
                .bytes()
                .map(|byte| match byte {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                        (byte as char).to_string()
                    }
                    _ => format!("%{:02X}", byte),
                })
                .collect()
        }

        format!("{}/{}", bucket, encode_segment(key))
    }

    async fn delete_directory_recursive(
        &self,
        bucket: &str,
        directory_path: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let dir_prefix = Self::normalize_directory_prefix(directory_path);
        let mut next_token: Option<String> = None;

        loop {
            let page = self
                .list_object_keys_page(bucket, &dir_prefix, next_token.as_deref())
                .await?;
            self.delete_objects_batch(bucket, &page.objects).await?;

            if page.next_token.is_none() {
                break;
            }
            next_token = page.next_token;
        }

        // 删除目录本身（如果存在作为对象的目录标记）
        if !dir_prefix.is_empty() {
            let _ = self._delete_object(bucket, &dir_prefix).await; // 忽略目录标记不存在的错误
        }

        Ok(())
    }
    pub async fn get_object_type(
        &self,
        bucket: &str,
        key: &str,
    ) -> Result<ObjectType, Box<dyn Error + Send + Sync>> {
        // 首先尝试获取对象信息
        let result = self.get_object(bucket, key).await;

        if result.is_ok() {
            // 如果能直接获取到对象，说明这是个普通对象
            return Ok(ObjectType::Object);
        }

        // 如果获取不到对象，检查是否有以此为前缀的目录
        let dir_key = if key.ends_with('/') {
            key.to_string()
        } else {
            format!("{}/", key)
        };

        let dir_result = self
            .list_objects(bucket, Some(&dir_key), Some(1), None)
            .await;
        if let Ok(dir_list) = dir_result {
            if !dir_list.objects.is_empty() {
                return Ok(ObjectType::Directory);
            }
        }

        // 检查原 key 加上 / 后是否是目录
        let alt_dir_key = format!("{}/", key);
        let alt_dir_result = self
            .list_objects(bucket, Some(&alt_dir_key), Some(1), None)
            .await;
        if let Ok(dir_list) = alt_dir_result {
            if !dir_list.objects.is_empty() {
                return Ok(ObjectType::Directory);
            }
        }

        Ok(ObjectType::NotFound)
    }

    pub async fn delete_object(
        &self,
        bucket: &str,
        key: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        match self.get_object_type(bucket, key).await? {
            ObjectType::Directory => {
                self.delete_directory_recursive(bucket, key).await?;
            }
            ObjectType::Object => {
                self._delete_object(bucket, key).await?;
            }
            ObjectType::NotFound => {
                // 尝试删除两种形式，以防万一
                let _ = self._delete_object(bucket, key).await?;
                let dir_key = if key.ends_with('/') {
                    key.to_string()
                } else {
                    format!("{}/", key)
                };
                let _ = self._delete_object(bucket, &dir_key).await?;
            }
        }

        Ok(())
    }

    fn replace_prefix(key: &str, source_prefix: &str, target_prefix: &str) -> String {
        key.strip_prefix(source_prefix)
            .map(|suffix| format!("{}{}", target_prefix, suffix))
            .unwrap_or_else(|| target_prefix.to_string())
    }

    pub async fn copy_object_or_directory(
        &self,
        bucket: &str,
        source_key: &str,
        target_key: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let source_key = source_key.trim_start_matches('/');
        let target_key = target_key.trim_start_matches('/');
        if source_key.is_empty() || target_key.is_empty() {
            return Err("源路径和目标路径不能为空".into());
        }
        if source_key == target_key {
            return Err("源路径和目标路径不能相同".into());
        }

        match self.get_object_type(bucket, source_key).await? {
            ObjectType::Directory => {
                let source_prefix = Self::normalize_directory_prefix(source_key);
                let target_prefix = Self::normalize_directory_prefix(target_key);
                let keys = self.list_all_object_keys(bucket, &source_prefix).await?;
                for key in keys {
                    let new_key = Self::replace_prefix(&key, &source_prefix, &target_prefix);
                    self.copy_single_object(bucket, &key, &new_key).await?;
                }
                self.create_directory(bucket, &target_prefix).await?;
            }
            ObjectType::Object => {
                self.copy_single_object(bucket, source_key, target_key)
                    .await?;
            }
            ObjectType::NotFound => return Err("源对象不存在".into()),
        }

        Ok(())
    }

    async fn copy_single_object(
        &self,
        bucket: &str,
        source_key: &str,
        target_key: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.client
            .copy_object()
            .bucket(bucket)
            .key(target_key)
            .copy_source(Self::encode_copy_source(bucket, source_key))
            .send()
            .await?;
        Ok(())
    }

    pub async fn move_object_or_directory(
        &self,
        bucket: &str,
        source_key: &str,
        target_key: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.copy_object_or_directory(bucket, source_key, target_key)
            .await?;
        self.delete_object(bucket, source_key).await?;
        Ok(())
    }

    fn remove_prefix(prefix: &str, key: &str) -> String {
        let mut name = key;
        // 如果传入了prefix，则从目录名中移除前缀
        if !prefix.is_empty() {
            // 确保只移除前缀部分，保留目录名称
            if key.starts_with(prefix) {
                name = name.strip_prefix(prefix).unwrap_or(key)
            }
        }
        name.to_string()
    }

    /**
     * 获取文件预签名URL
     * @param key 文件名
     * @param expiresIn 签名有效期
     */
    pub async fn get_presigned_url(
        &self,
        bucket: &str,
        key: &str,
        expires_in: Duration,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let presigning_config = PresigningConfig::builder().expires_in(expires_in).build()?;

        let presigned_request = self
            .client
            .get_object()
            .bucket(bucket)
            .key(key)
            .presigned(presigning_config)
            .await?;

        Ok(presigned_request.uri().to_string())
    }

    /**
     * 滚动获取文件列表
     * @param prefix 文件前缀
     * @param max_keys 最大数量
     * @param next_token 下一页标记
     */
    pub async fn list_objects(
        &self,
        bucket: &str,
        prefix: Option<&str>,
        max_keys: Option<i32>,
        next_token: Option<&str>,
    ) -> Result<FileList, SdkError<ListObjectsV2Error, HttpResponse>> {
        let _max_keys = if let Some(v) = max_keys { v } else { 100 };
        let prefix = prefix.unwrap_or("");
        let mut build = self
            .client
            .list_objects_v2()
            .bucket(bucket)
            .delimiter("/")
            .max_keys(_max_keys);
        if !prefix.is_empty() {
            build = build.prefix(prefix);
        }
        if let Some(next_token) = next_token {
            if !next_token.is_empty() {
                build = build.continuation_token(next_token);
            }
        }
        let mut res = FileList {
            objects: vec![],
            next_token: None,
        };
        let response = build.send().await?;
        if let Some(v) = response.next_continuation_token() {
            res.next_token = Some(v.to_string());
        }
        for x in response.common_prefixes() {
            res.objects.push(FileInfo {
                name: Self::remove_prefix(prefix, x.prefix().unwrap().strip_suffix("/").unwrap()),
                last_modified: None,
                size: None,
                is_dir: true,
                content_type: None,
            })
        }
        for x in response.contents() {
            let content_type = mime_guess::from_path(x.key().unwrap())
                .first_or_octet_stream()
                .essence_str()
                .to_string();
            res.objects.push(FileInfo {
                name: Self::remove_prefix(prefix, x.key().unwrap()),
                last_modified: Option::from(x.last_modified.unwrap().to_string()),
                size: x.size,
                is_dir: false,
                content_type: Option::from(content_type),
            })
        }
        Ok(res)
    }

    pub async fn list_all_object_keys(
        &self,
        bucket: &str,
        prefix: &str,
    ) -> Result<Vec<String>, SdkError<ListObjectsV2Error, HttpResponse>> {
        let mut result = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let page = self
                .list_object_keys_page(bucket, prefix, next_token.as_deref())
                .await?;
            result.extend(page.objects);

            if page.next_token.is_none() {
                break;
            }
            next_token = page.next_token;
        }

        Ok(result)
    }

    async fn list_object_keys_page(
        &self,
        bucket: &str,
        prefix: &str,
        next_token: Option<&str>,
    ) -> Result<ObjectKeyPage, SdkError<ListObjectsV2Error, HttpResponse>> {
        let mut build = self
            .client
            .list_objects_v2()
            .bucket(bucket)
            .prefix(prefix)
            .max_keys(1000);
        if let Some(next_token) = next_token {
            if !next_token.is_empty() {
                build = build.continuation_token(next_token);
            }
        }

        let response = build.send().await?;
        let objects = response
            .contents()
            .iter()
            .filter_map(|object| object.key().map(|key| key.to_string()))
            .collect();

        Ok(ObjectKeyPage {
            objects,
            next_token: response.next_continuation_token().map(|v| v.to_string()),
        })
    }

    pub async fn list_buckets(&self) -> Result<Vec<BucketInfo>, Box<dyn Error + Send + Sync>> {
        let response = self.client.list_buckets().send().await?;
        let buckets = response
            .buckets()
            .iter()
            .filter_map(|bucket| {
                bucket.name().map(|name| BucketInfo {
                    name: name.to_string(),
                    creation_date: bucket.creation_date().map(|date| date.to_string()),
                })
            })
            .collect();

        Ok(buckets)
    }

    pub async fn probe_permissions(
        &self,
        bucket: &str,
    ) -> Result<BucketPermissions, Box<dyn Error + Send + Sync>> {
        let probe_key = format!(".s3-client-permission-probe/{}.tmp", uuid::Uuid::new_v4());
        let list_result = self.list_objects(bucket, Some(""), Some(1), None).await;
        let list = list_result.is_ok();
        let delete = self._delete_object(bucket, &probe_key).await.is_ok();

        let read_existing = if let Ok(file_list) = list_result {
            if let Some(file) = file_list.objects.iter().find(|file| !file.is_dir) {
                self.get_object(bucket, &file.name).await.is_ok()
            } else {
                false
            }
        } else {
            false
        };

        let write = if delete {
            self.client
                .put_object()
                .bucket(bucket)
                .key(&probe_key)
                .body(ByteStream::from_static(b""))
                .content_length(0)
                .send()
                .await
                .is_ok()
        } else {
            false
        };

        let read = if write {
            self.get_object(bucket, &probe_key).await.is_ok()
        } else {
            read_existing
        };

        if write {
            let _ = self._delete_object(bucket, &probe_key).await;
        }

        Ok(BucketPermissions {
            list,
            read,
            write,
            delete,
        })
    }

    /**
     * 上传文件
     * @param key 文件存储key
     * @param file_path 文件本地路径
     * @param progress_callback 进度回调
     */
    pub async fn upload_file(
        &self,
        bucket: &str,
        key: &str,
        file_path: &str,
        progress_callback: Option<Box<dyn Fn(u64, u64) + Send>>,
        cancellation_token: CancellationToken,
    ) -> Result<(), String> {
        if cancellation_token.is_cancelled() {
            return Err("任务已取消".to_string());
        }

        // 获取文件元数据
        let metadata = tokio::fs::metadata(file_path)
            .await
            .map_err(|e| format!("获取文件元数据失败: {}", e))?;
        let total_size = metadata.len();

        // 对于大文件，使用分片上传 如果提供了进度回调，则使用带进度的上传
        // 标准S3协议规定除最后一个分片外，每个分片大小不能小于5MB
        if progress_callback.is_some() && total_size > 5 * 1024 * 1024 {
            // 5MB 以上使用分片上传
            return self
                .upload_file_multipart(
                    bucket,
                    key,
                    file_path,
                    total_size,
                    progress_callback.unwrap(),
                    cancellation_token,
                )
                .await
                .map_err(|e| return format!("分段上传失败: {}", e));
        }

        if cancellation_token.is_cancelled() {
            return Err("任务已取消".to_string());
        }

        let _res = self
            .put_object(bucket, key, file_path)
            .await
            .map_err(|e| format!("文件上传失败: {}", e))?;
        // 调用最终进度回调
        if let Some(callback) = progress_callback {
            callback(total_size, total_size);
        }
        Ok(())
    }

    async fn upload_file_multipart(
        &self,
        bucket: &str,
        key: &str,
        file_path: &str,
        total_size: u64,
        progress_callback: Box<dyn Fn(u64, u64) + Send + 'static>,
        cancellation_token: CancellationToken,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // 开始分片上传
        let create_multipart_upload_output = self
            .client
            .create_multipart_upload()
            .bucket(bucket)
            .key(key)
            .send()
            .await?;

        let upload_id = create_multipart_upload_output
            .upload_id()
            .ok_or("分段上传未返回 upload_id")?;

        let mut part_number = 1;
        let mut uploaded_bytes = 0u64;
        let mut completed_parts = Vec::new();

        // 设置分段大小（5MB）
        let chunk_size: usize = 5 * 1024 * 1024;

        let mut file = AsyncFile::open(file_path).await?;

        loop {
            if cancellation_token.is_cancelled() {
                let _ = self
                    .client
                    .abort_multipart_upload()
                    .bucket(bucket)
                    .key(key)
                    .upload_id(upload_id)
                    .send()
                    .await;
                return Err("任务已取消".into());
            }

            // 读取一块数据
            let mut buffer = vec![0; chunk_size];
            let mut bytes_read = 0;

            // 循环读取，直到填满 chunk_size 或者触及文件末尾
            while bytes_read < chunk_size {
                // read每次只能读取2MB字节
                let n = file.read(&mut buffer[bytes_read..]).await?;
                if n == 0 {
                    break; // 到达文件末尾
                }
                bytes_read += n;
            }
            if bytes_read == 0 {
                break; // 文件读取完毕
            }
            if cancellation_token.is_cancelled() {
                let _ = self
                    .client
                    .abort_multipart_upload()
                    .bucket(bucket)
                    .key(key)
                    .upload_id(upload_id)
                    .send()
                    .await;
                return Err("任务已取消".into());
            }
            // 调整缓冲区大小
            buffer.truncate(bytes_read);
            // 上传分片
            let upload_part_output = self
                .client
                .upload_part()
                .bucket(bucket)
                .key(key)
                .part_number(part_number)
                .upload_id(upload_id)
                .body(ByteStream::from(buffer))
                .send()
                .await?;
            let etag = upload_part_output
                .e_tag()
                .map(|t| t.trim_matches('"').to_string())
                .ok_or("上传分片未返回 ETag")?;
            completed_parts.push(
                aws_sdk_s3::types::CompletedPart::builder()
                    .part_number(part_number)
                    .e_tag(etag)
                    .build(),
            );
            // 更新已上传字节数
            uploaded_bytes += bytes_read as u64;
            // 调用进度回调
            progress_callback(total_size, uploaded_bytes);
            part_number += 1;
            if bytes_read < chunk_size {
                break; // 已读完文件
            }
        }
        completed_parts.sort_by_key(|part| part.part_number);
        // 完成分段上传
        let completed_multipart_upload = aws_sdk_s3::types::CompletedMultipartUpload::builder()
            .set_parts(Some(completed_parts))
            .build();

        let output = self
            .client
            .complete_multipart_upload()
            .bucket(bucket)
            .key(key)
            .upload_id(upload_id)
            .multipart_upload(completed_multipart_upload)
            .send()
            .await;
        match output {
            Ok(_) => {}
            Err(e) => {
                let _ = self
                    .client
                    .abort_multipart_upload()
                    .bucket(bucket)
                    .key(key)
                    .upload_id(upload_id)
                    .send()
                    .await;
                if let Some(service_error) = e.as_service_error() {
                    // 这里会打印具体错误，比如：Code: EntityTooSmall
                    eprintln!("MinIO 错误代码: {:?}", service_error.code());
                    eprintln!("错误详情: {:?}", service_error.message());
                } else {
                    eprintln!("其他错误: {:?}", e);
                }
                return Err(format!("完成分段上传失败: {}", e).into());
            }
        }
        Ok(())
    }

    /**
     * 下载文件
     * @param key: 文件对象Key
     * @param file_path 文件本地路径
     * @param progress_callback 进度回调
     */
    pub async fn download_file(
        &self,
        bucket: &str,
        key: &str,
        file_path: &str,
        progress_callback: Box<dyn Fn(u64, u64) + Send>,
        cancellation_token: CancellationToken,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if cancellation_token.is_cancelled() {
            return Err("任务已取消".into());
        }

        // 确保本地目录存在
        tokio::fs::create_dir_all(Path::new(file_path).parent().unwrap())
            .await
            .map_err(|e| format!("创建目录失败: {}", e))?;

        // 创建本地文件
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(file_path)
            .await
            .map_err(|e| format!("创建本地文件失败: {}", e))?;

        // 获取远程文件信息
        let result = self.get_object(bucket, key).await?;
        // 获取文件大小
        let file_size = result.content_length().unwrap_or(0) as u64;

        // 设置分片大小（2MB）
        let chunk_size: u64 = 2 * 1024 * 1024;
        if file_size <= chunk_size {
            if cancellation_token.is_cancelled() {
                return Err("任务已取消".into());
            }
            let byte_result = result.body.collect().await?.to_vec();
            if cancellation_token.is_cancelled() {
                return Err("任务已取消".into());
            }
            // 对于小文件，直接下载
            file.write_all(&byte_result).await?;
            file.flush().await?;
            // 调用最终进度回调
            progress_callback(file_size, file_size);
            Ok(())
        } else {
            let mut downloaded_bytes = 0u64;

            while downloaded_bytes < file_size {
                if cancellation_token.is_cancelled() {
                    return Err("任务已取消".into());
                }
                let end_byte = std::cmp::min(downloaded_bytes + chunk_size - 1, file_size - 1);
                let range_header = format!("bytes={}-{}", downloaded_bytes, end_byte);

                // 获取分片数据
                let response = self
                    .client
                    .get_object()
                    .bucket(bucket)
                    .key(key)
                    .range(range_header)
                    .send()
                    .await
                    .map_err(|e| format!("下载文件分片失败: {}", e))?;

                let body = response
                    .body
                    .collect()
                    .await
                    .map_err(|e| format!("读取文件分片内容失败: {}", e))?;

                let chunk_data = body.into_bytes();

                if cancellation_token.is_cancelled() {
                    return Err("任务已取消".into());
                }

                // 写入本地文件
                file.write_all(&chunk_data)
                    .await
                    .map_err(|e| format!("写入文件分片失败: {}", e))?;

                // 更新已下载字节数
                let chunk_len = chunk_data.len() as u64;
                downloaded_bytes += chunk_len;
                // 调用最终进度回调
                progress_callback(file_size, downloaded_bytes);
            }
            // 确保文件写入完成
            file.flush()
                .await
                .map_err(|e| format!("刷新文件缓存失败: {}", e))?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Oss;

    #[test]
    fn normalizes_directory_prefix_for_recursive_delete() {
        assert_eq!(Oss::normalize_directory_prefix("dir"), "dir/");
        assert_eq!(Oss::normalize_directory_prefix("dir/"), "dir/");
        assert_eq!(Oss::normalize_directory_prefix("/dir/sub/"), "dir/sub/");
        assert_eq!(Oss::normalize_directory_prefix(""), "");
        assert_eq!(Oss::normalize_directory_prefix("/"), "");
    }

    #[test]
    fn chunks_delete_keys_by_s3_limit() {
        let keys = (0..2501)
            .map(|index| format!("file-{}.txt", index))
            .collect::<Vec<_>>();
        let chunks = Oss::chunk_delete_keys(&keys);

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].len(), 1000);
        assert_eq!(chunks[1].len(), 1000);
        assert_eq!(chunks[2].len(), 501);
    }
}
