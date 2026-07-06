use crate::models::OssConfig;
use aes_gcm::aead::rand_core::{OsRng, RngCore};
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use rusqlite::{params, Connection};
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::{LazyLock, OnceLock};
use tauri::{AppHandle, Manager};

const DB_FILE_NAME: &str = "config.sqlite";
const KEY_FILE_NAME: &str = "secret.key";

pub static APP_CONFIG: LazyLock<Mutex<AppConfig>> =
    LazyLock::new(|| -> Mutex<AppConfig> { Mutex::new(AppConfig::new()) });
pub static GLOBAL_APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

#[derive(Debug, Default, Clone)]
pub struct AppConfig {
    pub oss: Vec<OssConfig>,
    db_path: Option<PathBuf>,
    key_path: Option<PathBuf>,
    init_error: Option<String>,
}

impl AppConfig {
    pub fn new() -> Self {
        Self::load().unwrap_or_else(|e| AppConfig {
            init_error: Some(e.to_string()),
            ..Default::default()
        })
    }

    fn load() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let config_dir = Self::get_config_dir()?;
        fs::create_dir_all(&config_dir)?;

        let db_path = config_dir.join(DB_FILE_NAME);
        let key_path = config_dir.join(KEY_FILE_NAME);
        let conn = Self::open_database(&db_path)?;

        let oss = Self::load_all(&conn, &key_path)?;

        Ok(AppConfig {
            oss,
            db_path: Some(db_path),
            key_path: Some(key_path),
            init_error: None,
        })
    }

    fn ensure_ready(&self) -> Result<(&PathBuf, &PathBuf), Box<dyn Error + Send + Sync>> {
        if let Some(e) = &self.init_error {
            return Err(e.clone().into());
        }

        let db_path = self.db_path.as_ref().ok_or("配置数据库未初始化")?;
        let key_path = self.key_path.as_ref().ok_or("配置密钥未初始化")?;
        Ok((db_path, key_path))
    }

    fn get_config_dir() -> Result<PathBuf, Box<dyn Error + Send + Sync>> {
        let handle = GLOBAL_APP_HANDLE.get().ok_or("应用尚未完成初始化")?;
        Ok(handle.path().app_config_dir()?)
    }

    fn open_database(db_path: &PathBuf) -> Result<Connection, Box<dyn Error + Send + Sync>> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS oss_configs (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                provider TEXT NOT NULL,
                access_key TEXT NOT NULL,
                secret_key_encrypted TEXT NOT NULL,
                endpoint TEXT NOT NULL,
                region TEXT NOT NULL,
                bucket TEXT NOT NULL,
                path_style TEXT NOT NULL
            );
            "#,
        )?;
        Ok(conn)
    }

    fn load_all(
        conn: &Connection,
        key_path: &PathBuf,
    ) -> Result<Vec<OssConfig>, Box<dyn Error + Send + Sync>> {
        let mut stmt = conn.prepare(
            r#"
            SELECT id, name, provider, access_key, secret_key_encrypted, endpoint, region, bucket, path_style
            FROM oss_configs
            ORDER BY name ASC
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
            ))
        })?;

        let mut configs = Vec::new();
        for row in rows {
            let (
                id,
                name,
                provider,
                access_key,
                secret_key_encrypted,
                endpoint,
                region,
                bucket,
                path_style,
            ) = row?;
            configs.push(OssConfig {
                id,
                name,
                provider,
                access_key,
                secret_key: Self::decrypt_secret(key_path, &secret_key_encrypted)?,
                endpoint,
                region,
                bucket,
                path_style,
            });
        }

        Ok(configs)
    }

    fn upsert_config(
        conn: &Connection,
        key_path: &PathBuf,
        config: &OssConfig,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let encrypted_secret = Self::encrypt_secret(key_path, &config.secret_key)?;
        conn.execute(
            r#"
            INSERT INTO oss_configs
                (id, name, provider, access_key, secret_key_encrypted, endpoint, region, bucket, path_style)
            VALUES
                (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                provider = excluded.provider,
                access_key = excluded.access_key,
                secret_key_encrypted = excluded.secret_key_encrypted,
                endpoint = excluded.endpoint,
                region = excluded.region,
                bucket = excluded.bucket,
                path_style = excluded.path_style
            "#,
            params![
                config.id,
                config.name,
                config.provider,
                config.access_key,
                encrypted_secret,
                config.endpoint,
                config.region,
                config.bucket,
                config.path_style
            ],
        )?;
        Ok(())
    }

    fn load_or_create_encryption_key(
        key_path: &PathBuf,
    ) -> Result<[u8; 32], Box<dyn Error + Send + Sync>> {
        if key_path.exists() {
            let encoded = fs::read_to_string(key_path)?;
            let decoded = BASE64.decode(encoded.trim())?;
            let key: [u8; 32] = decoded.try_into().map_err(|_| "配置加密密钥长度不正确")?;
            return Ok(key);
        }

        if let Some(parent) = key_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        fs::write(key_path, BASE64.encode(key))?;
        Ok(key)
    }

    fn encrypt_secret(
        key_path: &PathBuf,
        secret: &str,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let key = Self::load_or_create_encryption_key(key_path)?;
        let cipher = Aes256Gcm::new_from_slice(&key)?;

        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, secret.as_bytes())
            .map_err(|_| "Secret Key 加密失败")?;

        Ok(format!(
            "{}:{}",
            BASE64.encode(nonce_bytes),
            BASE64.encode(ciphertext)
        ))
    }

    fn decrypt_secret(
        key_path: &PathBuf,
        encrypted: &str,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let (nonce_encoded, ciphertext_encoded) = encrypted
            .split_once(':')
            .ok_or("Secret Key 密文格式不正确")?;
        let nonce_bytes = BASE64.decode(nonce_encoded)?;
        let ciphertext = BASE64.decode(ciphertext_encoded)?;
        let key = Self::load_or_create_encryption_key(key_path)?;
        let cipher = Aes256Gcm::new_from_slice(&key)?;
        let plaintext = cipher
            .decrypt(Nonce::from_slice(&nonce_bytes), ciphertext.as_ref())
            .map_err(|_| "Secret Key 解密失败")?;
        Ok(String::from_utf8(plaintext)?)
    }

    pub fn get(&self, id: &str) -> Result<&OssConfig, Box<dyn Error + Send + Sync>> {
        self.ensure_ready()?;
        self.oss
            .iter()
            .find(|config| config.id == id)
            .ok_or_else(|| "未找到该配置".into())
    }

    pub fn list(&self) -> Result<Vec<OssConfig>, Box<dyn Error + Send + Sync>> {
        self.ensure_ready()?;
        Ok(self.oss.clone())
    }

    pub fn remove(&mut self, id: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let (db_path, key_path) = {
            let (db_path, key_path) = self.ensure_ready()?;
            (db_path.clone(), key_path.clone())
        };
        let conn = Self::open_database(&db_path)?;
        let affected = conn.execute("DELETE FROM oss_configs WHERE id = ?1", params![id])?;
        if affected == 0 {
            return Err("未找到该配置".into());
        }
        self.oss = Self::load_all(&conn, &key_path)?;
        Ok(())
    }

    pub fn update(&mut self, config: OssConfig) -> Result<(), Box<dyn Error + Send + Sync>> {
        let (db_path, key_path) = {
            let (db_path, key_path) = self.ensure_ready()?;
            (db_path.clone(), key_path.clone())
        };
        let conn = Self::open_database(&db_path)?;
        Self::upsert_config(&conn, &key_path, &config)?;
        self.oss = Self::load_all(&conn, &key_path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::AppConfig;
    use std::fs;

    #[test]
    fn encrypts_and_decrypts_secret_key() {
        let key_path = std::env::temp_dir().join(format!(
            "s3-client-test-secret-{}.key",
            uuid::Uuid::new_v4()
        ));
        let secret = "very-secret-access-key";

        let encrypted = AppConfig::encrypt_secret(&key_path, secret).unwrap();
        assert_ne!(encrypted, secret);
        assert_eq!(
            AppConfig::decrypt_secret(&key_path, &encrypted).unwrap(),
            secret
        );

        let _ = fs::remove_file(key_path);
    }
}
