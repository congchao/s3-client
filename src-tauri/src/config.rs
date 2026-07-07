use crate::models::OssConfig;
use aes_gcm::aead::rand_core::{OsRng, RngCore};
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextMenuSettings {
    pub download: bool,
    pub rename: bool,
    pub move_item: bool,
    pub duplicate: bool,
    pub share: bool,
    pub delete: bool,
    pub copy_path: bool,
    pub parquet_to_excel: bool,
}

impl Default for ContextMenuSettings {
    fn default() -> Self {
        Self {
            download: true,
            rename: true,
            move_item: true,
            duplicate: true,
            share: true,
            delete: true,
            copy_path: true,
            parquet_to_excel: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub file_context_menu: ContextMenuSettings,
    pub directory_context_menu: ContextMenuSettings,
}

#[derive(Debug, Clone)]
pub struct ReleaseVersionRecord {
    pub version: String,
    pub tag_name: String,
    pub name: Option<String>,
    pub release_url: Option<String>,
    pub body: Option<String>,
    pub published_at: Option<String>,
}

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
                path_style TEXT NOT NULL,
                sort INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS app_settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                file_download INTEGER NOT NULL,
                file_rename INTEGER NOT NULL,
                file_move_item INTEGER NOT NULL,
                file_duplicate INTEGER NOT NULL,
                file_share INTEGER NOT NULL,
                file_delete INTEGER NOT NULL,
                file_copy_path INTEGER NOT NULL,
                file_parquet_to_excel INTEGER NOT NULL,
                directory_download INTEGER NOT NULL,
                directory_rename INTEGER NOT NULL,
                directory_move_item INTEGER NOT NULL,
                directory_duplicate INTEGER NOT NULL,
                directory_share INTEGER NOT NULL,
                directory_delete INTEGER NOT NULL,
                directory_copy_path INTEGER NOT NULL,
                directory_parquet_to_excel INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS release_versions (
                version TEXT PRIMARY KEY NOT NULL,
                tag_name TEXT NOT NULL,
                name TEXT,
                release_url TEXT,
                body TEXT,
                published_at TEXT,
                checked_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                skipped INTEGER NOT NULL DEFAULT 0
            );
            "#,
        )?;
        Self::ensure_oss_sort_column(&conn)?;
        Ok(conn)
    }

    fn ensure_oss_sort_column(conn: &Connection) -> Result<(), Box<dyn Error + Send + Sync>> {
        let has_sort = conn
            .prepare("PRAGMA table_info(oss_configs)")?
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?
            .iter()
            .any(|column| column == "sort");

        if !has_sort {
            conn.execute(
                "ALTER TABLE oss_configs ADD COLUMN sort INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }

        let zero_sort_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM oss_configs WHERE sort = 0",
            [],
            |row| row.get(0),
        )?;
        let total_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM oss_configs", [], |row| row.get(0))?;
        if total_count > 1 && zero_sort_count == total_count {
            let ids = conn
                .prepare("SELECT id FROM oss_configs ORDER BY name ASC")?
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            for (index, id) in ids.iter().enumerate() {
                conn.execute(
                    "UPDATE oss_configs SET sort = ?1 WHERE id = ?2",
                    params![((index + 1) as i64) * 1000, id],
                )?;
            }
        }

        Ok(())
    }

    fn load_all(
        conn: &Connection,
        key_path: &PathBuf,
    ) -> Result<Vec<OssConfig>, Box<dyn Error + Send + Sync>> {
        let mut stmt = conn.prepare(
            r#"
            SELECT id, name, provider, access_key, secret_key_encrypted, endpoint, region, bucket, path_style, sort
            FROM oss_configs
            ORDER BY sort ASC, name ASC
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
                row.get::<_, i64>(9)?,
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
                sort,
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
                sort,
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
        let sort = if config.sort > 0 {
            config.sort
        } else {
            Self::next_sort(conn)?
        };
        conn.execute(
            r#"
            INSERT INTO oss_configs
                (id, name, provider, access_key, secret_key_encrypted, endpoint, region, bucket, path_style, sort)
            VALUES
                (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                provider = excluded.provider,
                access_key = excluded.access_key,
                secret_key_encrypted = excluded.secret_key_encrypted,
                endpoint = excluded.endpoint,
                region = excluded.region,
                bucket = excluded.bucket,
                path_style = excluded.path_style,
                sort = excluded.sort
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
                config.path_style,
                sort
            ],
        )?;
        Ok(())
    }

    fn next_sort(conn: &Connection) -> Result<i64, Box<dyn Error + Send + Sync>> {
        let max_sort: i64 = conn.query_row(
            "SELECT COALESCE(MAX(sort), 0) FROM oss_configs",
            [],
            |row| row.get(0),
        )?;
        Ok(max_sort + 1000)
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

    pub fn update_sort_order(
        &mut self,
        ids: Vec<String>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let (db_path, key_path) = {
            let (db_path, key_path) = self.ensure_ready()?;
            (db_path.clone(), key_path.clone())
        };
        let mut conn = Self::open_database(&db_path)?;
        let tx = conn.transaction()?;
        for (index, id) in ids.iter().enumerate() {
            tx.execute(
                "UPDATE oss_configs SET sort = ?1 WHERE id = ?2",
                params![((index + 1) as i64) * 1000, id],
            )?;
        }
        tx.commit()?;
        self.oss = Self::load_all(&conn, &key_path)?;
        Ok(())
    }

    fn bool_to_i64(value: bool) -> i64 {
        if value {
            1
        } else {
            0
        }
    }

    fn i64_to_bool(value: i64) -> bool {
        value != 0
    }

    pub fn get_settings(&self) -> Result<AppSettings, Box<dyn Error + Send + Sync>> {
        let (db_path, _) = self.ensure_ready()?;
        let conn = Self::open_database(db_path)?;
        let mut stmt = conn.prepare(
            r#"
            SELECT
                file_download, file_rename, file_move_item, file_duplicate, file_share, file_delete, file_copy_path, file_parquet_to_excel,
                directory_download, directory_rename, directory_move_item, directory_duplicate, directory_share, directory_delete, directory_copy_path, directory_parquet_to_excel
            FROM app_settings
            WHERE id = 1
            "#,
        )?;

        let result = stmt.query_row([], |row| {
            Ok(AppSettings {
                file_context_menu: ContextMenuSettings {
                    download: Self::i64_to_bool(row.get(0)?),
                    rename: Self::i64_to_bool(row.get(1)?),
                    move_item: Self::i64_to_bool(row.get(2)?),
                    duplicate: Self::i64_to_bool(row.get(3)?),
                    share: Self::i64_to_bool(row.get(4)?),
                    delete: Self::i64_to_bool(row.get(5)?),
                    copy_path: Self::i64_to_bool(row.get(6)?),
                    parquet_to_excel: Self::i64_to_bool(row.get(7)?),
                },
                directory_context_menu: ContextMenuSettings {
                    download: Self::i64_to_bool(row.get(8)?),
                    rename: Self::i64_to_bool(row.get(9)?),
                    move_item: Self::i64_to_bool(row.get(10)?),
                    duplicate: Self::i64_to_bool(row.get(11)?),
                    share: Self::i64_to_bool(row.get(12)?),
                    delete: Self::i64_to_bool(row.get(13)?),
                    copy_path: Self::i64_to_bool(row.get(14)?),
                    parquet_to_excel: Self::i64_to_bool(row.get(15)?),
                },
            })
        });

        match result {
            Ok(settings) => Ok(settings),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(AppSettings::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save_settings(&self, settings: AppSettings) -> Result<(), Box<dyn Error + Send + Sync>> {
        let (db_path, _) = self.ensure_ready()?;
        let conn = Self::open_database(db_path)?;
        conn.execute(
            r#"
            INSERT INTO app_settings (
                id,
                file_download, file_rename, file_move_item, file_duplicate, file_share, file_delete, file_copy_path, file_parquet_to_excel,
                directory_download, directory_rename, directory_move_item, directory_duplicate, directory_share, directory_delete, directory_copy_path, directory_parquet_to_excel
            )
            VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
            ON CONFLICT(id) DO UPDATE SET
                file_download = excluded.file_download,
                file_rename = excluded.file_rename,
                file_move_item = excluded.file_move_item,
                file_duplicate = excluded.file_duplicate,
                file_share = excluded.file_share,
                file_delete = excluded.file_delete,
                file_copy_path = excluded.file_copy_path,
                file_parquet_to_excel = excluded.file_parquet_to_excel,
                directory_download = excluded.directory_download,
                directory_rename = excluded.directory_rename,
                directory_move_item = excluded.directory_move_item,
                directory_duplicate = excluded.directory_duplicate,
                directory_share = excluded.directory_share,
                directory_delete = excluded.directory_delete,
                directory_copy_path = excluded.directory_copy_path,
                directory_parquet_to_excel = excluded.directory_parquet_to_excel
            "#,
            params![
                Self::bool_to_i64(settings.file_context_menu.download),
                Self::bool_to_i64(settings.file_context_menu.rename),
                Self::bool_to_i64(settings.file_context_menu.move_item),
                Self::bool_to_i64(settings.file_context_menu.duplicate),
                Self::bool_to_i64(settings.file_context_menu.share),
                Self::bool_to_i64(settings.file_context_menu.delete),
                Self::bool_to_i64(settings.file_context_menu.copy_path),
                Self::bool_to_i64(settings.file_context_menu.parquet_to_excel),
                Self::bool_to_i64(settings.directory_context_menu.download),
                Self::bool_to_i64(settings.directory_context_menu.rename),
                Self::bool_to_i64(settings.directory_context_menu.move_item),
                Self::bool_to_i64(settings.directory_context_menu.duplicate),
                Self::bool_to_i64(settings.directory_context_menu.share),
                Self::bool_to_i64(settings.directory_context_menu.delete),
                Self::bool_to_i64(settings.directory_context_menu.copy_path),
                Self::bool_to_i64(settings.directory_context_menu.parquet_to_excel),
            ],
        )?;
        Ok(())
    }

    pub fn record_release_version(
        &self,
        release: ReleaseVersionRecord,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let (db_path, _) = self.ensure_ready()?;
        let conn = Self::open_database(db_path)?;
        conn.execute(
            r#"
            INSERT INTO release_versions (
                version, tag_name, name, release_url, body, published_at, checked_at, skipped
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, CURRENT_TIMESTAMP, 0)
            ON CONFLICT(version) DO UPDATE SET
                tag_name = excluded.tag_name,
                name = excluded.name,
                release_url = excluded.release_url,
                body = excluded.body,
                published_at = excluded.published_at,
                checked_at = CURRENT_TIMESTAMP,
                skipped = release_versions.skipped
            "#,
            params![
                release.version,
                release.tag_name,
                release.name,
                release.release_url,
                release.body,
                release.published_at,
            ],
        )?;
        Ok(())
    }

    pub fn is_release_skipped(&self, version: &str) -> Result<bool, Box<dyn Error + Send + Sync>> {
        let (db_path, _) = self.ensure_ready()?;
        let conn = Self::open_database(db_path)?;
        let skipped = conn.query_row(
            "SELECT skipped FROM release_versions WHERE version = ?1",
            params![version],
            |row| row.get::<_, i64>(0),
        );

        match skipped {
            Ok(value) => Ok(Self::i64_to_bool(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    pub fn skip_release_version(&self, version: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let (db_path, _) = self.ensure_ready()?;
        let conn = Self::open_database(db_path)?;
        conn.execute(
            r#"
            INSERT INTO release_versions (version, tag_name, checked_at, skipped)
            VALUES (?1, ?1, CURRENT_TIMESTAMP, 1)
            ON CONFLICT(version) DO UPDATE SET skipped = 1
            "#,
            params![version],
        )?;
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
