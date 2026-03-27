use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)] // 启用默认值（字段缺失时使用）
#[serde(rename_all = "camelCase")] // 添加此行，使serde能接收驼峰命名
pub struct OssConfig {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub access_key: String,
    pub secret_key: String,
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub path_style: String,
}

// 为 OssConfig 实现默认值
impl Default for OssConfig {
    fn default() -> Self {
        OssConfig {
            id: "".to_string(),
            name: "".to_string(),
            provider: "".to_string(),
            access_key: "".to_string(),
            secret_key: "".to_string(),
            endpoint: "".to_string(),
            region: "cn-north-1".to_string(),
            bucket: "".to_string(),
            path_style: "".to_string(),
        }
    }
}
