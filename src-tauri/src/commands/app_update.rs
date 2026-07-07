use crate::config::{ReleaseVersionRecord, APP_CONFIG};
use reqwest::header::{ACCEPT, USER_AGENT};
use semver::Version;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;
use url::Url;

const GITHUB_RELEASES_URL: &str = "https://api.github.com/repos/congchao/s3-client/releases";
const UPDATER_MANIFEST_URL: &str =
    "https://github.com/congchao/s3-client/releases/latest/download/latest.json";
const USER_AGENT_VALUE: &str = "s3-client-updater";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct GitHubRelease {
    tag_name: String,
    name: Option<String>,
    html_url: Option<String>,
    body: Option<String>,
    published_at: Option<String>,
    draft: bool,
    prerelease: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateCheckResult {
    current_version: String,
    latest_version: String,
    latest_tag: String,
    release_name: Option<String>,
    release_url: Option<String>,
    published_at: Option<String>,
    update_available: bool,
    skipped: bool,
    should_prompt: bool,
}

#[tauri::command]
pub async fn app_update_check(
    app: AppHandle,
    interactive: bool,
) -> Result<AppUpdateCheckResult, String> {
    let current_version = app.package_info().version.to_string();
    let releases = fetch_releases().await?;
    let release = releases
        .iter()
        .find(|release| !release.draft && !release.prerelease)
        .ok_or_else(|| "没有找到可用于更新的正式 Release".to_string())?;
    let latest_version = normalize_version(&release.tag_name);
    {
        let app_config = APP_CONFIG.lock().map_err(|e| e.to_string())?;
        for release in releases
            .iter()
            .filter(|release| !release.draft && !release.prerelease)
        {
            app_config
                .record_release_version(ReleaseVersionRecord {
                    version: normalize_version(&release.tag_name),
                    tag_name: release.tag_name.clone(),
                    name: release.name.clone(),
                    release_url: release.html_url.clone(),
                    body: release.body.clone(),
                    published_at: release.published_at.clone(),
                })
                .map_err(|e| e.to_string())?;
        }
    }

    let update_available = is_newer_version(&latest_version, &current_version);
    let skipped = {
        let app_config = APP_CONFIG.lock().map_err(|e| e.to_string())?;
        app_config
            .is_release_skipped(&latest_version)
            .map_err(|e| e.to_string())?
    };

    Ok(AppUpdateCheckResult {
        current_version,
        latest_version,
        latest_tag: release.tag_name.clone(),
        release_name: release.name.clone(),
        release_url: release.html_url.clone(),
        published_at: release.published_at.clone(),
        update_available,
        skipped,
        should_prompt: update_available && (interactive || !skipped),
    })
}

#[tauri::command]
pub async fn app_update_skip(version: String) -> Result<(), String> {
    let app_config = APP_CONFIG.lock().map_err(|e| e.to_string())?;
    app_config
        .skip_release_version(&version)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn app_update_install(app: AppHandle) -> Result<(), String> {
    let endpoint = Url::parse(UPDATER_MANIFEST_URL).map_err(|e| e.to_string())?;
    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;

    let update = updater
        .check()
        .await
        .map_err(|e| format!("检查更新清单失败: {}", e))?;
    let Some(update) = update else {
        return Err("当前已经是最新版本".to_string());
    };

    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| format!("下载或安装更新失败: {}", e))?;
    app.restart();
}

async fn fetch_releases() -> Result<Vec<GitHubRelease>, String> {
    let client = reqwest::Client::new();
    let releases = client
        .get(GITHUB_RELEASES_URL)
        .query(&[("per_page", "30")])
        .header(USER_AGENT, USER_AGENT_VALUE)
        .header(ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("获取 GitHub Releases 失败: {}", e))?
        .error_for_status()
        .map_err(|e| format!("GitHub Releases 请求失败: {}", e))?
        .json::<Vec<GitHubRelease>>()
        .await
        .map_err(|e| format!("解析 GitHub Releases 失败: {}", e))?;

    Ok(releases)
}

fn normalize_version(version: &str) -> String {
    version
        .trim()
        .trim_start_matches('v')
        .trim_start_matches('V')
        .to_string()
}

fn is_newer_version(latest: &str, current: &str) -> bool {
    match (
        Version::parse(&normalize_version(latest)),
        Version::parse(&normalize_version(current)),
    ) {
        (Ok(latest), Ok(current)) => latest > current,
        _ => normalize_version(latest) != normalize_version(current),
    }
}
