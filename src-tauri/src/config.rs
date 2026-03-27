use crate::models::OssConfig;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::{LazyLock, OnceLock};
use tauri::{AppHandle, Manager};

pub static APP_CONFIG: LazyLock<Mutex<AppConfig>> =
    LazyLock::new(|| -> Mutex<AppConfig> { Mutex::new(AppConfig::new()) });
pub static GLOBAL_APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

// ======================== 配置结构体定义 ========================
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
#[serde(default)] // 启用默认值（字段缺失时使用）
pub struct AppConfig {
    // OSS 配置列表
    pub oss: Vec<OssConfig>,
}

// 为 AppConfig 实现默认值
impl AppConfig {
    fn get_config_path() -> PathBuf {
        GLOBAL_APP_HANDLE
            .get()
            .unwrap()
            .path()
            .app_config_dir()
            .unwrap()
            .join("config.json")
    }
    pub fn new() -> Self {
        let path = Self::get_config_path();
        // 若文件不存在，创建默认配置文件并返回默认值
        if !path.exists() {
            println!("配置文件不存在，创建默认配置文件: {:?}", path);
            let default_config = AppConfig::default();
            // 确保父目录存在
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("TODO: panic message");
            }
            // 写入默认配置
            let json_str =
                serde_json::to_string_pretty(&default_config).expect("TODO: panic message");
            fs::write(path, json_str).expect("TODO: panic message");
            return default_config;
        }

        // 读取文件内容并反序列化
        let content = fs::read_to_string(path).expect("TODO: panic message");
        serde_json::from_str(&content).expect("TODO: panic message")
    }

    pub fn get(&self, id: &str) -> Result<&OssConfig, Box<dyn Error>> {
        for x in &self.oss {
            if x.id == id {
                return Ok(&x);
            }
        }
        Err("未找到该配置".into())
    }

    /// 将当前全局配置写回配置文件
    fn save(&self) -> Result<(), Box<dyn Error>> {
        // 读取当前全局配置
        // 序列化为格式化的 JSON
        let json_str = serde_json::to_string_pretty(&self)?;
        // 写入文件（覆盖原有内容）
        fs::write(Self::get_config_path(), json_str)?;
        Ok(())
    }

    pub fn remove(&mut self, id: &str) -> Result<(), Box<dyn Error>> {
        for (i, x) in self.oss.iter().enumerate() {
            if x.id == id {
                self.oss.remove(i);
                self.save()?;
                return Ok(());
            }
        }
        Err("未找到该配置".into())
    }

    pub fn update(&mut self, config: OssConfig) -> Result<(), Box<dyn Error>> {
        match self.get(&config.id) {
            Ok(_) => {
                // 配置存在，进行更新
                for (i, x) in self.oss.iter_mut().enumerate() {
                    if x.id == config.id {
                        self.oss[i] = config;
                        self.save()?;
                        return Ok(());
                    }
                }
            }
            Err(_) => {
                // 配置不存在，添加新配置
                self.oss.push(config);
                self.save()?;
            }
        }
        Ok(())
    }
}
