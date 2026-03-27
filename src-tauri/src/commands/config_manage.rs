use crate::config::APP_CONFIG;
use crate::models::OssConfig;
use crate::utils::Oss;

// 配置管理命令
#[tauri::command]
pub async fn config_save(config: OssConfig) -> Result<(), String> {
    let mut app_config = APP_CONFIG.lock().map_err(|e| e.to_string())?;
    app_config.update(config).expect("更新失败");
    Ok(())
}

#[tauri::command]
pub async fn config_get() -> Result<Vec<OssConfig>, String> {
    let app_config = APP_CONFIG.lock().map_err(|e| e.to_string())?;
    Ok(app_config.oss.clone())
}

#[tauri::command]
pub async fn config_delete(id: String) -> Result<(), String> {
    let mut app_config = APP_CONFIG
        .lock()
        .map_err(|e| format!("配置锁错误: {}", e))?;

    match app_config.remove(&id) {
        Ok(()) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

// 连接测试命令
#[tauri::command]
pub async fn config_test(config: OssConfig) -> Result<bool, String> {
    let oss = Oss::new_with_config(&config);

    match oss {
        Ok(oss_instance) => match oss_instance.list_objects(Option::from(""), Option::from(1), None).await {
            Ok(_) => {
                println!("连接测试成功: {}", config.name);
                Ok(true)
            }
            Err(e) => {
                println!("连接测试失败: {}", e);
                Err(format!("连接测试失败: {}", e))
            }
        },
        Err(e) => {
            println!("创建OSS实例失败: {}", e);
            Err(format!("创建OSS实例失败: {}", e))
        }
    }
}
