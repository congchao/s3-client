use crate::models::TransferProgress;
use crate::utils::{FileList, Oss};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::fs;
use tokio::sync::{mpsc, Semaphore};
use uuid::Uuid;

// 限制最大并发数为 5
const MAX_CONCURRENT: usize = 5;

// 全局发送端，用于将任务发送给后台消费者
struct TransferSystem {
    upload_tx: mpsc::UnboundedSender<(TransferProgress, AppHandle)>,
    download_tx: mpsc::UnboundedSender<(TransferProgress, AppHandle)>,
}

// 使用 LazyLock 初始化全局单例
static TRANSFER_SYSTEM: LazyLock<TransferSystem> = LazyLock::new(|| {
    let (upload_tx, mut upload_rx) = mpsc::unbounded_channel::<(TransferProgress, AppHandle)>();
    let (download_tx, mut download_rx) = mpsc::unbounded_channel::<(TransferProgress, AppHandle)>();

    tokio::spawn(async move {
        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT));
        while let Some((task, app)) = upload_rx.recv().await {
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            tokio::spawn(async move {
                // 执行上传
                if let Err(e) = perform_upload(task.clone(), app).await {
                    eprintln!("上传任务失败: {}, 错误: {}", task.from_path, e);
                }
                // 任务结束，Drop permit 自动释放信号量
                drop(permit);
            });
        }
    });

    tokio::spawn(async move {
        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT));
        while let Some((task, app)) = download_rx.recv().await {
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            tokio::spawn(async move {
                // 执行下载
                if let Err(e) = perform_download(task.clone(), app).await {
                    eprintln!("下载任务失败: {}, 错误: {}", task.from_path, e);
                }
                drop(permit);
            });
        }
    });

    TransferSystem {
        upload_tx,
        download_tx,
    }
});

async fn perform_upload(mut task: TransferProgress, app: AppHandle) -> Result<(), String> {
    let oss = Oss::new(&task.config_id).map_err(|e| e.to_string())?;

    // 发送开始状态
    task.status = "uploading".to_string();
    let _ = app.emit("transfer_process", &task);

    let task_clone = task.clone();
    let app_clone = app.clone();

    let result = oss
        .upload_file(
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
                    v.status = "completed".to_string();
                }
                // 忽略 emit 错误，防止日志刷屏
                let _ = app_clone.emit("transfer_process", v);
            })),
        )
        .await;

    if let Err(e) = result {
        task.status = "failed".to_string();
        let _ = app.emit("transfer_process", task);
        return Err(format!("上传失败: {}", e));
    }
    Ok(())
}

async fn perform_download(mut task: TransferProgress, app: AppHandle) -> Result<(), String> {
    let oss = Oss::new(&task.config_id).map_err(|e| e.to_string())?;

    // 发送开始状态
    task.status = "downloading".to_string();
    let _ = app.emit("transfer_process", &task);

    let task_clone = task.clone();
    let app_clone = app.clone();

    let result = oss
        .download_file(
            &task.from_path,
            &task.to_path,
            Box::new(move |total, current| {
                let mut v = task_clone.clone();
                if total > 0 {
                    v.progress = (current as f64 / total as f64) * 100.0;
                }
                if v.progress >= 100.0 {
                    v.progress = 100.0;
                    v.status = "completed".to_string();
                }
                let _ = app_clone.emit("transfer_process", v);
            }),
        )
        .await;

    if let Err(e) = result {
        task.status = "failed".to_string();
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
                .to_string_lossy()
                .to_string();

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

#[tauri::command]
pub async fn file_list(
    id: String,
    path: Option<String>,
    next_token: Option<String>,
) -> Result<FileList, String> {
    let oss = Oss::new(&id).map_err(|e| format!("OSS init failed: {}", e))?;
    oss.list_objects(path.as_deref(), Some(1000), next_token.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn file_download(id: String, path: String) -> Result<Vec<u8>, String> {
    let oss = Oss::new(&id).map_err(|e| e.to_string())?;
    let response = oss.get_object(&path).await.map_err(|e| e.to_string())?;
    let body = response.body.collect().await.map_err(|e| e.to_string())?;
    Ok(body.into_bytes().to_vec())
}

#[tauri::command]
pub async fn file_delete(id: String, key: String) -> Result<(), String> {
    let oss = Oss::new(&id).map_err(|e| e.to_string())?;
    oss.delete_object(&key).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn file_get_preview_url(id: String, key: String) -> Result<String, String> {
    let oss = Oss::new(&id).map_err(|e| e.to_string())?;
    oss.get_presigned_url(&key, Duration::from_secs(3600))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn file_upload(
    id: String,
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

    let mut tasks = Vec::new();

    for (full_path, rel_path) in files_to_process {
        // 2. 拼接最终 OSS Key
        // 如果 remote_path 是 "uploads"，rel_path 是 "data/metrics/a.json"
        // 结果就是 "uploads/data/metrics/a.json"
        let file_name = Path::new(&full_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        let final_key = if remote_path.is_empty() {
            rel_path.clone()
        } else {
            format!(
                "{}/{}",
                remote_path.strip_suffix("/").unwrap_or(&remote_path),
                rel_path
            )
        };

        let task = TransferProgress {
            id: Uuid::new_v4().to_string(),
            config_id: id.clone(),
            name: file_name.to_string(),
            from_path: full_path.clone(),
            to_path: final_key,
            size: fs::metadata(&full_path).await.map(|m| m.len()).unwrap_or(0),
            progress: 0.0,
            status: "waiting".to_string(),
        };
        tasks.push(task);
    }

    // 3. 将任务推送到全局通道
    for task in &tasks {
        let _ = TRANSFER_SYSTEM
            .upload_tx
            .send((task.clone(), app_handle.clone()));
    }

    // 4. 立即返回任务列表给前端，让前端渲染列表
    Ok(tasks)
}

#[tauri::command]
pub async fn file_download_path(
    id: String,
    remote_keys: Vec<String>,
    local_path: String,
    app_handle: AppHandle,
) -> Result<Vec<TransferProgress>, String> {
    if remote_keys.is_empty() {
        return Err("请选择要下载的文件".to_string());
    }

    let mut tasks = Vec::new();
    let oss = Oss::new(&id).map_err(|e| e.to_string())?;

    // 1. 展开所有文件 (如果是文件夹)
    for remote_key in remote_keys {
        // 注意：这里需要确定 remote_basic 用于计算相对路径
        let remote_basic = Path::new(&remote_key)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        if remote_key.ends_with('/') {
            // 是目录，递归列出（这里保留递归查找，但建议后端OSS有前缀搜索能力）
            let all = oss.list_all_objects(&remote_key).await;
            for obj in all.unwrap().objects {
                if !obj.is_dir {
                    tasks.push(create_download_task(
                        &id,
                        &obj.name,
                        &remote_basic,
                        &local_path,
                    ));
                }
            }
        } else {
            tasks.push(create_download_task(
                &id,
                &remote_key,
                &remote_basic,
                &local_path,
            ));
        }
    }

    // 2. 创建本地目录结构 (预创建，防止并发写入同一目录时的竞争)
    // 实际创建目录建议放到 perform_download 内部或者这里统一处理

    // 3. 发送到下载队列
    for task in &tasks {
        let _ = TRANSFER_SYSTEM
            .download_tx
            .send((task.clone(), app_handle.clone()));
    }

    Ok(tasks)
}

fn create_download_task(
    id: &str,
    key: &str,
    base_remote: &str,
    local_root: &str,
) -> TransferProgress {
    // 简单的路径计算
    let relative = key.trim_start_matches(base_remote).trim_start_matches('/');
    let local_dest = Path::new(local_root).join(relative);

    TransferProgress {
        id: Uuid::new_v4().to_string(),
        config_id: id.to_string(),
        name: Path::new(key)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        from_path: key.to_string(),
        to_path: local_dest.to_string_lossy().to_string(),
        size: 0,
        progress: 0.0,
        status: "waiting".to_string(),
    }
}
