use aws_config::timeout::TimeoutConfig;
use aws_config::Region;
use aws_sdk_s3::config::http::HttpResponse;
use aws_sdk_s3::config::{Credentials, SharedCredentialsProvider};
use aws_sdk_s3::error::SdkError;
use aws_sdk_s3::operation::delete_object::*;
use aws_sdk_s3::operation::get_object::*;
use aws_sdk_s3::operation::list_buckets::*;
use aws_sdk_s3::operation::list_objects_v2::*;
use aws_sdk_s3::operation::put_object::*;
// 添加预签名相关导入
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::Client;
use aws_smithy_types::byte_stream::ByteStream;

use crate::config::APP_CONFIG;
use crate::models::OssConfig;
use aws_smithy_types::checksum_config::RequestChecksumCalculation;
use aws_smithy_types::error::metadata::ProvideErrorMetadata;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::Path;
use std::time::Duration;
use tokio::fs::File as AsyncFile;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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

pub struct Oss {
    pub client: Client,
    pub config: OssConfig,
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
            config: oss_config.clone(),
        })
    }

    pub fn new(id: &str) -> Result<Self, Box<dyn Error>> {
        let cnf = APP_CONFIG.lock().unwrap();
        let oss_config = cnf.get(id).cloned()?;
        Ok(Self::new_with_config(&oss_config)?)
    }

    pub async fn list_buckets(
        &self,
    ) -> Result<ListBucketsOutput, SdkError<ListBucketsError, HttpResponse>> {
        self.client.list_buckets().send().await
    }

    pub async fn get_object(
        &self,
        key: &str,
    ) -> Result<GetObjectOutput, SdkError<GetObjectError, HttpResponse>> {
        self.client
            .get_object()
            .bucket(&self.config.bucket)
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
        key: &str,
        path: &str,
    ) -> Result<PutObjectOutput, SdkError<PutObjectError, HttpResponse>> {
        // 创建字节流时添加更好的错误处理
        let byte_stream = ByteStream::from_path(path)
            .await
            .map_err(|e| println!("创建字节流失败: {}，路径: {}", e, path))
            .unwrap();
        let content_length = tokio::fs::metadata(path).await.unwrap().len() as i64;
        // 根据文件路径推断 content-type
        let mime_type = mime_guess::from_path(path).first_or_octet_stream();
        let content_type = mime_type.essence_str().to_string();
        self.client
            .put_object()
            .bucket(&self.config.bucket)
            .key(key)
            .body(byte_stream)
            .content_type(content_type)
            .content_length(content_length)
            .send()
            .await
    }

    async fn _delete_object(
        &self,
        key: &str,
    ) -> Result<DeleteObjectOutput, SdkError<DeleteObjectError, HttpResponse>> {
        self.client
            .delete_object()
            .bucket(&self.config.bucket)
            .key(key)
            .send()
            .await
    }
    async fn delete_directory_recursive(
        &self,
        directory_path: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let dir_prefix = if directory_path.ends_with('/') {
            directory_path.to_string()
        } else {
            format!("{}/", directory_path)
        };

        // 获取目录下所有对象
        let mut all_objects = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let file_list = self
                .list_objects(Some(&dir_prefix), Some(1000), next_token.as_deref())
                .await?;

            for file_info in file_list.objects {
                if !file_info.is_dir {
                    let full_key = if dir_prefix.is_empty() {
                        file_info.name.clone()
                    } else {
                        format!("{}{}", dir_prefix, file_info.name)
                    };
                    all_objects.push(full_key);
                }
            }

            if file_list.next_token.is_none() {
                break;
            }
            next_token = file_list.next_token;
        }

        // 批量删除所有对象
        for object_key in all_objects {
            self._delete_object(&object_key).await?;
        }

        // 删除目录本身（如果存在作为对象的目录标记）
        if !dir_prefix.is_empty() {
            let _ = self._delete_object(&dir_prefix).await; // 忽略目录标记不存在的错误
        }

        Ok(())
    }
    pub async fn get_object_type(
        &self,
        key: &str,
    ) -> Result<ObjectType, Box<dyn Error + Send + Sync>> {
        // 首先尝试获取对象信息
        let result = self.get_object(key).await;

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

        let dir_result = self.list_objects(Some(&dir_key), Some(1), None).await;
        if let Ok(dir_list) = dir_result {
            if !dir_list.objects.is_empty() {
                return Ok(ObjectType::Directory);
            }
        }

        // 检查原 key 加上 / 后是否是目录
        let alt_dir_key = format!("{}/", key);
        let alt_dir_result = self.list_objects(Some(&alt_dir_key), Some(1), None).await;
        if let Ok(dir_list) = alt_dir_result {
            if !dir_list.objects.is_empty() {
                return Ok(ObjectType::Directory);
            }
        }

        Ok(ObjectType::NotFound)
    }

    pub async fn delete_object(&self, key: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        match self.get_object_type(key).await? {
            ObjectType::Directory => {
                self.delete_directory_recursive(key).await?;
            }
            ObjectType::Object => {
                self._delete_object(key).await?;
            }
            ObjectType::NotFound => {
                // 尝试删除两种形式，以防万一
                let _ = self._delete_object(key).await?;
                let dir_key = if key.ends_with('/') {
                    key.to_string()
                } else {
                    format!("{}/", key)
                };
                let _ = self._delete_object(&dir_key).await?;
            }
        }

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
        key: &str,
        expires_in: Duration,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let presigning_config = PresigningConfig::builder().expires_in(expires_in).build()?;

        let presigned_request = self
            .client
            .get_object()
            .bucket(&self.config.bucket)
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
        prefix: Option<&str>,
        max_keys: Option<i32>,
        next_token: Option<&str>,
    ) -> Result<FileList, SdkError<ListObjectsV2Error, HttpResponse>> {
        let _max_keys = if let Some(v) = max_keys { v } else { 100 };
        let mut build = self
            .client
            .list_objects_v2()
            .bucket(&self.config.bucket)
            .delimiter("/")
            .max_keys(_max_keys);
        if !prefix.is_none() && prefix.unwrap() != "" {
            build = build.prefix(prefix.unwrap());
        }
        if !next_token.is_none() && next_token.unwrap() != "" {
            build = build.continuation_token(next_token.unwrap());
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
                name: Self::remove_prefix(
                    prefix.unwrap(),
                    x.prefix().unwrap().strip_suffix("/").unwrap(),
                ),
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
                name: Self::remove_prefix(prefix.unwrap(), x.key().unwrap()),
                last_modified: Option::from(x.last_modified.unwrap().to_string()),
                size: x.size,
                is_dir: false,
                content_type: Option::from(content_type),
            })
        }
        Ok(res)
    }

    /**
     * 递归目录列出指定prefix下的所有文件
     * @param prefix 目录前缀
     */
    pub async fn list_all_objects(
        &self,
        prefix: &String,
    ) -> Result<FileList, SdkError<ListObjectsV2Error, HttpResponse>> {
        async fn list_all_objects_recursive(
            client: &Client,
            bucket: &str,
            prefix: &str,
            next_token: Option<&str>,
        ) -> Result<FileList, SdkError<ListObjectsV2Error, HttpResponse>> {
            let mut build = client
                .list_objects_v2()
                .bucket(bucket)
                .prefix(prefix)
                .max_keys(1000);
            if !next_token.is_none() && next_token.unwrap() != "" {
                build = build.continuation_token(next_token.unwrap());
            }
            let mut res = FileList {
                objects: vec![],
                next_token: None,
            };
            let response = build.send().await?;
            if let Some(v) = response.next_continuation_token() {
                res.next_token = Some(v.to_string());
            }
            for x in response.contents() {
                let content_type = mime_guess::from_path(x.key().unwrap())
                    .first_or_octet_stream()
                    .essence_str()
                    .to_string();
                res.objects.push(FileInfo {
                    name: x.key.clone().unwrap(),
                    last_modified: Option::from(x.last_modified.unwrap().to_string()),
                    size: x.size,
                    is_dir: false,
                    content_type: Option::from(content_type),
                })
            }
            Ok(res)
        }
        let mut result = FileList {
            objects: vec![],
            next_token: None,
        };
        loop {
            let res = list_all_objects_recursive(
                &self.client,
                &self.config.bucket,
                prefix,
                result.next_token.clone().as_deref(),
            )
            .await?;
            result.next_token = res.next_token.clone();
            for object in res.objects {
                result.objects.push(object);
            }
            if result.next_token.is_none() {
                break;
            }
        }
        Ok(result)
    }

    /**
     * 上传文件
     * @param key 文件存储key
     * @param file_path 文件本地路径
     * @param progress_callback 进度回调
     */
    pub async fn upload_file(
        &self,
        key: &str,
        file_path: &str,
        progress_callback: Option<Box<dyn Fn(u64, u64) + Send>>,
    ) -> Result<(), String> {
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
                .upload_file_multipart(key, file_path, total_size, progress_callback.unwrap())
                .await
                .map_err(|e| return format!("分段上传失败: {}", e));
        }

        let _res = self
            .put_object(key, file_path)
            .await
            .map_err(|e| format!("文件上传失败: {}", e));
        // 调用最终进度回调
        if let Some(callback) = progress_callback {
            callback(total_size, total_size);
        }
        Ok(())
    }

    async fn upload_file_multipart(
        &self,
        key: &str,
        file_path: &str,
        total_size: u64,
        progress_callback: Box<dyn Fn(u64, u64) + Send + 'static>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // 开始分片上传
        let create_multipart_upload_output = self
            .client
            .create_multipart_upload()
            .bucket(&self.config.bucket)
            .key(key)
            .send()
            .await?;

        let upload_id = create_multipart_upload_output.upload_id().unwrap();

        let mut part_number = 1;
        let mut uploaded_bytes = 0u64;
        let mut completed_parts = Vec::new();

        // 设置分段大小（5MB）
        let chunk_size: usize = 5 * 1024 * 1024;

        let mut file = AsyncFile::open(file_path).await?;

        loop {
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
            // 调整缓冲区大小
            buffer.truncate(bytes_read);
            // 上传分片
            let upload_part_output = self
                .client
                .upload_part()
                .bucket(&self.config.bucket)
                .key(key)
                .part_number(part_number)
                .upload_id(upload_id)
                .body(ByteStream::from(buffer))
                .send()
                .await?;
            let etag = upload_part_output
                .e_tag()
                .map(|t| t.trim_matches('"').to_string())
                .unwrap();
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
            .bucket(&self.config.bucket)
            .key(key)
            .upload_id(upload_id)
            .multipart_upload(completed_multipart_upload)
            .send()
            .await;
        match output {
            Ok(_) => {}
            Err(e) => {
                if let Some(service_error) = e.as_service_error() {
                    // 这里会打印具体错误，比如：Code: EntityTooSmall
                    eprintln!("MinIO 错误代码: {:?}", service_error.code().unwrap());
                    eprintln!("错误详情: {:?}", service_error.message().unwrap());
                } else {
                    eprintln!("其他错误: {:?}", e);
                }
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
        key: &str,
        file_path: &str,
        progress_callback: Box<dyn Fn(u64, u64) + Send>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
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
        let result = self.get_object(key).await.unwrap();
        // 获取文件大小
        let file_size = result.content_length().unwrap_or(0) as u64;

        // 设置分片大小（2MB）
        let chunk_size: u64 = 2 * 1024 * 1024;
        if file_size <= chunk_size {
            let byte_result = result.body.collect().await.unwrap().to_vec();
            // 对于小文件，直接下载
            file.write_all(&byte_result).await?;
            file.flush().await?;
            // 调用最终进度回调
            progress_callback(file_size, file_size);
            Ok(())
        } else {
            let mut downloaded_bytes = 0u64;

            while downloaded_bytes < file_size {
                let end_byte = std::cmp::min(downloaded_bytes + chunk_size - 1, file_size - 1);
                let range_header = format!("bytes={}-{}", downloaded_bytes, end_byte);

                // 获取分片数据
                let response = self
                    .client
                    .get_object()
                    .bucket(&self.config.bucket)
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
