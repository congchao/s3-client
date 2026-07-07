import {invoke} from "@tauri-apps/api/core";
import type {AppSettings, AppUpdateCheckResult, OssConfig} from "@/types";

// 获取配置列表
const getConfig = async (): Promise<OssConfig[]> => {
    return await invoke<OssConfig[]>('config_get');
};

// 保存配置
const saveConfig = async (config: OssConfig): Promise<void> => {
    return await invoke<void>('config_save', {config});
};

const saveConfigSort = async (ids: string[]): Promise<void> => {
    return await invoke<void>('config_sort_save', {ids});
};

// 删除配置
const deleteConfig = async (id: string): Promise<void> => {
    return await invoke<void>('config_delete', {id});
};

// 测试配置连接
const testConfig = async (config: OssConfig): Promise<boolean> => {
    return await invoke<boolean>('config_test', {config});
};

const getSettings = async (): Promise<AppSettings> => {
    return await invoke<AppSettings>('settings_get');
};

const saveSettings = async (settings: AppSettings): Promise<void> => {
    return await invoke<void>('settings_save', {settings});
};

const checkUpdate = async (interactive: boolean): Promise<AppUpdateCheckResult> => {
    return await invoke<AppUpdateCheckResult>('app_update_check', {interactive});
};

const skipUpdate = async (version: string): Promise<void> => {
    return await invoke<void>('app_update_skip', {version});
};

const installUpdate = async (): Promise<void> => {
    return await invoke<void>('app_update_install');
};

export const configApi = {
    getConfig,
    saveConfig,
    saveConfigSort,
    deleteConfig,
    testConfig,
    getSettings,
    saveSettings,
    checkUpdate,
    skipUpdate,
    installUpdate
};
