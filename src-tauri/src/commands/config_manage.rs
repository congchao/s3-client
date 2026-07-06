use crate::config::APP_CONFIG;
use crate::models::OssConfig;
use crate::utils::Oss;

// 配置管理命令
#[tauri::command]
pub async fn config_save(config: OssConfig) -> Result<(), String> {
    let config_id = config.id.clone();
    let mut app_config = APP_CONFIG.lock().map_err(|e| e.to_string())?;
    app_config.update(config).map_err(|e| e.to_string())?;
    Oss::clear_cached_config(&config_id);
    Ok(())
}

#[tauri::command]
pub async fn config_get() -> Result<Vec<OssConfig>, String> {
    let app_config = APP_CONFIG.lock().map_err(|e| e.to_string())?;
    app_config.list().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn config_delete(id: String) -> Result<(), String> {
    let mut app_config = APP_CONFIG
        .lock()
        .map_err(|e| format!("配置锁错误: {}", e))?;

    match app_config.remove(&id) {
        Ok(()) => {
            Oss::clear_cached_config(&id);
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}

// 连接测试命令
#[tauri::command]
pub async fn config_test(config: OssConfig) -> Result<bool, String> {
    let oss = Oss::new_with_config(&config);

    match oss {
        Ok(oss_instance) => {
            let result: Result<(), Box<dyn std::error::Error + Send + Sync>> =
                if config.bucket.trim().is_empty() {
                    oss_instance.list_buckets().await.map(|_| ())
                } else {
                    oss_instance
                        .list_objects(&config.bucket, Option::from(""), Option::from(1), None)
                        .await
                        .map(|_| ())
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                };

            match result {
                Ok(_) => {
                    println!("连接测试成功: {}", config.name);
                    Ok(true)
                }
                Err(e) => {
                    println!("连接测试失败: {}", e);
                    Err(format!("连接测试失败: {}", e))
                }
            }
        }
        Err(e) => {
            println!("创建OSS实例失败: {}", e);
            Err(format!("创建OSS实例失败: {}", e))
        }
    }
}
