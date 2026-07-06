use crate::models::{TransferProgress, TransferStatus};
use crate::utils::{BucketInfo, BucketPermissions, FileList, Oss};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::fs;
use tokio::sync::{mpsc, Semaphore};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

// 限制最大并发数为 5
const MAX_CONCURRENT: usize = 5;
const TRANSFER_QUEUE_CAPACITY: usize = 1000;
const PREVIEW_DOWNLOAD_MAX_SIZE: i64 = 10 * 1024 * 1024;

struct TransferJob {
    task: TransferProgress,
    app: AppHandle,
    cancellation_token: CancellationToken,
}

// 全局发送端，用于将任务发送给后台消费者
struct TransferSystem {
    upload_tx: mpsc::Sender<TransferJob>,
    download_tx: mpsc::Sender<TransferJob>,
}

static TRANSFER_CANCELLATIONS: LazyLock<Mutex<HashMap<String, CancellationToken>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// 使用 LazyLock 初始化全局单例
static TRANSFER_SYSTEM: LazyLock<TransferSystem> = LazyLock::new(|| {
    let (upload_tx, mut upload_rx) = mpsc::channel::<TransferJob>(TRANSFER_QUEUE_CAPACITY);
    let (download_tx, mut download_rx) = mpsc::channel::<TransferJob>(TRANSFER_QUEUE_CAPACITY);

    tokio::spawn(async move {
        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT));
        while let Some(job) = upload_rx.recv().await {
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            tokio::spawn(async move {
                let TransferJob {
                    task,
                    app,
                    cancellation_token,
                } = job;
                // 执行上传
                if let Err(e) = perform_upload(task.clone(), app, cancellation_token).await {
                    eprintln!("上传任务失败: {}, 错误: {}", task.from_path, e);
                }
                unregister_transfer_task(&task.id);
                // 任务结束，Drop permit 自动释放信号量
                drop(permit);
            });
        }
    });

    tokio::spawn(async move {
        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT));
        while let Some(job) = download_rx.recv().await {
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            tokio::spawn(async move {
                let TransferJob {
                    task,
                    app,
                    cancellation_token,
                } = job;
                // 执行下载
                if let Err(e) = perform_download(task.clone(), app, cancellation_token).await {
                    eprintln!("下载任务失败: {}, 错误: {}", task.from_path, e);
                }
                unregister_transfer_task(&task.id);
                drop(permit);
            });
        }
    });

    TransferSystem {
        upload_tx,
        download_tx,
    }
});

fn register_transfer_task(task_id: &str) -> Result<CancellationToken, String> {
    let token = CancellationToken::new();
    let mut cancellations = TRANSFER_CANCELLATIONS
        .lock()
        .map_err(|e| format!("传输任务状态锁错误: {}", e))?;
    cancellations.insert(task_id.to_string(), token.clone());
    Ok(token)
}

fn unregister_transfer_task(task_id: &str) {
    if let Ok(mut cancellations) = TRANSFER_CANCELLATIONS.lock() {
        cancellations.remove(task_id);
    }
}

fn emit_cancelled(mut task: TransferProgress, app: AppHandle) {
    task.status = TransferStatus::Cancelled;
    let _ = app.emit("transfer_process", task);
}

async fn perform_upload(
    mut task: TransferProgress,
    app: AppHandle,
    cancellation_token: CancellationToken,
) -> Result<(), String> {
    if cancellation_token.is_cancelled() {
        emit_cancelled(task, app);
        return Ok(());
    }

    let oss = Oss::new_cached(&task.config_id).map_err(|e| e.to_string())?;

    // 发送开始状态
    task.status = TransferStatus::Uploading;
    let _ = app.emit("transfer_process", &task);

    let task_clone = task.clone();
    let app_clone = app.clone();

    let result = oss
        .upload_file(
            &task.bucket,
            &task.to_path,
            &task.from_path,
            Some(Box::new(move |total, current| {
                let mut v = task_clone.clone();
                // 防止除以0
                if total > 0 {
                    v.progress = (current as f64 / total as f64) * 100.0;
                }
                if v.progress >= 100.0 {
                    v.progress = 100.0;
                    v.status = TransferStatus::Completed;
                }
                // 忽略 emit 错误，防止日志刷屏
                let _ = app_clone.emit("transfer_process", v);
            })),
            cancellation_token.clone(),
        )
        .await;

    if let Err(e) = result {
        if cancellation_token.is_cancelled() {
            emit_cancelled(task, app);
            return Ok(());
        }
        task.status = TransferStatus::Failed;
        let _ = app.emit("transfer_process", task);
        return Err(format!("上传失败: {}", e));
    }
    Ok(())
}

async fn perform_download(
    mut task: TransferProgress,
    app: AppHandle,
    cancellation_token: CancellationToken,
) -> Result<(), String> {
    if cancellation_token.is_cancelled() {
        emit_cancelled(task, app);
        return Ok(());
    }

    let oss = Oss::new_cached(&task.config_id).map_err(|e| e.to_string())?;

    // 发送开始状态
    task.status = TransferStatus::Downloading;
    let _ = app.emit("transfer_process", &task);

    let task_clone = task.clone();
    let app_clone = app.clone();

    let result = oss
        .download_file(
            &task.bucket,
            &task.from_path,
            &task.to_path,
            Box::new(move |total, current| {
                let mut v = task_clone.clone();
                if total > 0 {
                    v.progress = (current as f64 / total as f64) * 100.0;
                }
                if v.progress >= 100.0 {
                    v.progress = 100.0;
                    v.status = TransferStatus::Completed;
                }
                let _ = app_clone.emit("transfer_process", v);
            }),
            cancellation_token.clone(),
        )
        .await;

    if let Err(e) = result {
        if cancellation_token.is_cancelled() {
            emit_cancelled(task, app);
            return Ok(());
        }
        task.status = TransferStatus::Failed;
        let _ = app.emit("transfer_process", task);
        return Err(format!("下载失败: {}", e));
    }
    Ok(())
}

async fn collect_files(root_paths: Vec<String>) -> Result<Vec<(String, String)>, std::io::Error> {
    let mut result = Vec::new(); // 存储 (完整路径, 相对路径)
    let mut stack = Vec::new();

    for p in root_paths {
        let root_path = PathBuf::from(&p);
        stack.push((root_path.clone(), root_path));
    }

    while let Some((current_path, base_root)) = stack.pop() {
        if current_path.is_file() {
            // 计算相对路径
            let rel_path = current_path
                .strip_prefix(base_root.parent().unwrap_or(&base_root)) // 保留选择的目录名本身
                .unwrap_or(&current_path)
                .components()
                .filter_map(|component| match component {
                    Component::Normal(part) => Some(part.to_string_lossy().to_string()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("/");

            result.push((current_path.to_string_lossy().to_string(), rel_path));
        } else if current_path.is_dir() {
            let mut entries = fs::read_dir(&current_path).await?;
            while let Some(entry) = entries.next_entry().await? {
                stack.push((entry.path(), base_root.clone()));
            }
        }
    }
    Ok(result)
}

fn build_upload_key(remote_path: &str, rel_path: &str) -> String {
    let remote_path = remote_path.trim_matches('/');
    if remote_path.is_empty() {
        rel_path.to_string()
    } else {
        format!("{}/{}", remote_path, rel_path.trim_start_matches('/'))
    }
}

fn parent_remote_prefix(key: &str) -> String {
    let normalized = key.trim_matches('/');
    normalized
        .rfind('/')
        .map(|index| normalized[..index].to_string())
        .unwrap_or_default()
}

fn relative_key<'a>(key: &'a str, base_remote: &str) -> &'a str {
    let key = key.trim_matches('/');
    let base_remote = base_remote.trim_matches('/');

    if base_remote.is_empty() {
        return key;
    }

    key.strip_prefix(base_remote)
        .and_then(|relative| relative.strip_prefix('/'))
        .unwrap_or(key)
}

fn safe_local_destination(
    local_root: &str,
    base_remote: &str,
    key: &str,
) -> Result<PathBuf, String> {
    let relative = relative_key(key, base_remote);
    if relative.is_empty() {
        return Err("下载路径不能为空".to_string());
    }

    let mut local_dest = PathBuf::from(local_root);
    for segment in relative.split('/') {
        if segment.is_empty()
            || segment == "."
            || segment == ".."
            || segment.contains('\\')
            || Path::new(segment).is_absolute()
        {
            return Err(format!("非法对象路径: {}", key));
        }
        local_dest.push(segment);
    }

    Ok(local_dest)
}

#[tauri::command]
pub async fn file_list(
    id: String,
    bucket: String,
    path: Option<String>,
    next_token: Option<String>,
) -> Result<FileList, String> {
    let oss = Oss::new_cached(&id).map_err(|e| format!("OSS init failed: {}", e))?;
    oss.list_objects(&bucket, path.as_deref(), Some(1000), next_token.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn file_download(id: String, bucket: String, path: String) -> Result<Vec<u8>, String> {
    let oss = Oss::new_cached(&id).map_err(|e| e.to_string())?;
    let response = oss
        .get_object(&bucket, &path)
        .await
        .map_err(|e| e.to_string())?;
    if response.content_length().unwrap_or(0) > PREVIEW_DOWNLOAD_MAX_SIZE {
        return Err("文件过大，请使用下载功能保存到本地".to_string());
    }
    let body = response.body.collect().await.map_err(|e| e.to_string())?;
    Ok(body.into_bytes().to_vec())
}

#[tauri::command]
pub async fn file_delete(id: String, bucket: String, key: String) -> Result<(), String> {
    let oss = Oss::new_cached(&id).map_err(|e| e.to_string())?;
    oss.delete_object(&bucket, &key)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn file_get_preview_url(
    id: String,
    bucket: String,
    key: String,
) -> Result<String, String> {
    let oss = Oss::new_cached(&id).map_err(|e| e.to_string())?;
    oss.get_presigned_url(&bucket, &key, Duration::from_secs(3600))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn file_create_presigned_url(
    id: String,
    bucket: String,
    key: String,
    expires_seconds: u64,
) -> Result<String, String> {
    if expires_seconds == 0 || expires_seconds > 7 * 24 * 60 * 60 {
        return Err("链接有效期必须在 1 秒到 7 天之间".to_string());
    }

    let oss = Oss::new_cached(&id).map_err(|e| e.to_string())?;
    oss.get_presigned_url(&bucket, &key, Duration::from_secs(expires_seconds))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn bucket_list(id: String) -> Result<Vec<BucketInfo>, String> {
    let oss = Oss::new_cached(&id).map_err(|e| e.to_string())?;
    oss.list_buckets().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn bucket_probe_permissions(
    id: String,
    bucket: String,
) -> Result<BucketPermissions, String> {
    let oss = Oss::new_cached(&id).map_err(|e| e.to_string())?;
    oss.probe_permissions(&bucket)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn file_create_directory(id: String, bucket: String, key: String) -> Result<(), String> {
    let oss = Oss::new_cached(&id).map_err(|e| e.to_string())?;
    oss.create_directory(&bucket, &key)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn file_copy(
    id: String,
    bucket: String,
    source_key: String,
    target_key: String,
) -> Result<(), String> {
    let oss = Oss::new_cached(&id).map_err(|e| e.to_string())?;
    oss.copy_object_or_directory(&bucket, &source_key, &target_key)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn file_move(
    id: String,
    bucket: String,
    source_key: String,
    target_key: String,
) -> Result<(), String> {
    let oss = Oss::new_cached(&id).map_err(|e| e.to_string())?;
    oss.move_object_or_directory(&bucket, &source_key, &target_key)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn file_upload(
    id: String,
    bucket: String,
    remote_path: String,
    local_path: Vec<String>,
    app_handle: AppHandle,
) -> Result<Vec<TransferProgress>, String> {
    let files_to_process = collect_files(local_path.clone())
        .await
        .map_err(|e| format!("扫描本地文件失败: {}", e))?;

    if files_to_process.is_empty() {
        return Err("未找到可上传的文件".to_string());
    }

    if files_to_process.len() > TRANSFER_QUEUE_CAPACITY {
        return Err(format!(
            "传输队列最多支持 {} 个等待任务",
            TRANSFER_QUEUE_CAPACITY
        ));
    }

    let mut tasks = Vec::new();

    for (full_path, rel_path) in files_to_process {
        // 2. 拼接最终 OSS Key
        // 如果 remote_path 是 "uploads"，rel_path 是 "data/metrics/a.json"
        // 结果就是 "uploads/data/metrics/a.json"
        let file_name = Path::new(&full_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        let final_key = build_upload_key(&remote_path, &rel_path);

        let task = TransferProgress {
            id: Uuid::new_v4().to_string(),
            config_id: id.clone(),
            bucket: bucket.clone(),
            name: file_name.to_string(),
            from_path: full_path.clone(),
            to_path: final_key,
            size: fs::metadata(&full_path).await.map(|m| m.len()).unwrap_or(0),
            progress: 0.0,
            status: TransferStatus::Waiting,
        };
        tasks.push(task);
    }

    // 3. 将任务推送到全局通道
    if tasks.len() > TRANSFER_SYSTEM.upload_tx.capacity() {
        return Err(format!(
            "上传队列剩余容量不足，当前最多还能添加 {} 个任务",
            TRANSFER_SYSTEM.upload_tx.capacity()
        ));
    }

    for task in &tasks {
        let cancellation_token = register_transfer_task(&task.id)?;
        if let Err(e) = TRANSFER_SYSTEM.upload_tx.try_send(TransferJob {
            task: task.clone(),
            app: app_handle.clone(),
            cancellation_token,
        }) {
            unregister_transfer_task(&task.id);
            return Err(format!("传输队列已满，无法添加上传任务: {}", e));
        }
    }

    // 4. 立即返回任务列表给前端，让前端渲染列表
    Ok(tasks)
}

#[tauri::command]
pub async fn file_download_path(
    id: String,
    bucket: String,
    remote_keys: Vec<String>,
    local_path: String,
    app_handle: AppHandle,
) -> Result<Vec<TransferProgress>, String> {
    if remote_keys.is_empty() {
        return Err("请选择要下载的文件".to_string());
    }

    let mut tasks = Vec::new();
    let oss = Oss::new_cached(&id).map_err(|e| e.to_string())?;

    // 1. 展开所有文件 (如果是文件夹)
    for remote_key in remote_keys {
        // 注意：这里需要确定 remote_basic 用于计算相对路径
        let remote_basic = parent_remote_prefix(&remote_key);
        if remote_key.ends_with('/') {
            // 是目录，递归列出（这里保留递归查找，但建议后端OSS有前缀搜索能力）
            let all_keys = oss
                .list_all_object_keys(&bucket, &remote_key)
                .await
                .map_err(|e| e.to_string())?;
            for key in all_keys {
                tasks.push(create_download_task(
                    &id,
                    &bucket,
                    &key,
                    &remote_basic,
                    &local_path,
                )?);
            }
        } else {
            tasks.push(create_download_task(
                &id,
                &bucket,
                &remote_key,
                &remote_basic,
                &local_path,
            )?);
        }
    }

    // 2. 创建本地目录结构 (预创建，防止并发写入同一目录时的竞争)
    // 实际创建目录建议放到 perform_download 内部或者这里统一处理

    // 3. 发送到下载队列
    if tasks.len() > TRANSFER_SYSTEM.download_tx.capacity() {
        return Err(format!(
            "下载队列剩余容量不足，当前最多还能添加 {} 个任务",
            TRANSFER_SYSTEM.download_tx.capacity()
        ));
    }

    for task in &tasks {
        let cancellation_token = register_transfer_task(&task.id)?;
        if let Err(e) = TRANSFER_SYSTEM.download_tx.try_send(TransferJob {
            task: task.clone(),
            app: app_handle.clone(),
            cancellation_token,
        }) {
            unregister_transfer_task(&task.id);
            return Err(format!("传输队列已满，无法添加下载任务: {}", e));
        }
    }

    Ok(tasks)
}

#[tauri::command]
pub async fn file_transfer_cancel(task_id: String) -> Result<(), String> {
    let cancellations = TRANSFER_CANCELLATIONS
        .lock()
        .map_err(|e| format!("传输任务状态锁错误: {}", e))?;
    let token = cancellations
        .get(&task_id)
        .ok_or_else(|| "未找到可取消的传输任务".to_string())?;
    token.cancel();
    Ok(())
}

#[tauri::command]
pub async fn file_transfer_retry(
    mut task: TransferProgress,
    transfer_type: String,
    app_handle: AppHandle,
) -> Result<TransferProgress, String> {
    task.progress = 0.0;
    task.status = TransferStatus::Waiting;

    let cancellation_token = register_transfer_task(&task.id)?;
    let job = TransferJob {
        task: task.clone(),
        app: app_handle,
        cancellation_token,
    };

    let result = match transfer_type.as_str() {
        "upload" => TRANSFER_SYSTEM.upload_tx.try_send(job),
        "download" => TRANSFER_SYSTEM.download_tx.try_send(job),
        _ => {
            unregister_transfer_task(&task.id);
            return Err("未知传输类型".to_string());
        }
    };

    if let Err(e) = result {
        unregister_transfer_task(&task.id);
        return Err(format!("传输队列已满，无法重试任务: {}", e));
    }

    Ok(task)
}

fn create_download_task(
    id: &str,
    bucket: &str,
    key: &str,
    base_remote: &str,
    local_root: &str,
) -> Result<TransferProgress, String> {
    let local_dest = safe_local_destination(local_root, base_remote, key)?;

    Ok(TransferProgress {
        id: Uuid::new_v4().to_string(),
        config_id: id.to_string(),
        bucket: bucket.to_string(),
        name: Path::new(key)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        from_path: key.to_string(),
        to_path: local_dest.to_string_lossy().to_string(),
        size: 0,
        progress: 0.0,
        status: TransferStatus::Waiting,
    })
}

#[cfg(test)]
mod tests {
    use super::{build_upload_key, safe_local_destination};
    use std::path::PathBuf;

    #[test]
    fn builds_upload_key_for_bucket_root() {
        assert_eq!(build_upload_key("", "file.txt"), "file.txt");
        assert_eq!(build_upload_key("/", "file.txt"), "file.txt");
    }

    #[test]
    fn builds_upload_key_for_nested_prefix() {
        assert_eq!(build_upload_key("dir", "file.txt"), "dir/file.txt");
        assert_eq!(build_upload_key("dir/", "file.txt"), "dir/file.txt");
        assert_eq!(
            build_upload_key("/dir/sub/", "file.txt"),
            "dir/sub/file.txt"
        );
    }

    #[test]
    fn builds_safe_download_path() {
        let path = safe_local_destination("/tmp/download", "dir", "dir/sub/file.txt").unwrap();
        assert_eq!(path, PathBuf::from("/tmp/download/sub/file.txt"));
    }

    #[test]
    fn rejects_unsafe_download_path() {
        assert!(safe_local_destination("/tmp/download", "", "../secret.txt").is_err());
        assert!(safe_local_destination("/tmp/download", "", "dir/../secret.txt").is_err());
        assert!(safe_local_destination("/tmp/download", "", "dir\\secret.txt").is_err());
    }
}
