import {invoke} from "@tauri-apps/api/core";
import {OssConfig} from "@/types";

// 获取配置列表
const getConfig = async (): Promise<OssConfig[]> => {
    return await invoke<OssConfig[]>('config_get');
};

// 保存配置
const saveConfig = async (config: OssConfig): Promise<void> => {
    return await invoke<void>('config_save', {config});
};

// 删除配置
const deleteConfig = async (id: string): Promise<void> => {
    return await invoke<void>('config_delete', {id});
};

// 测试配置连接
const testConfig = async (config: OssConfig): Promise<boolean> => {
    return await invoke<boolean>('config_test', {config});
};

export const configApi = {
    getConfig,
    saveConfig,
    deleteConfig,
    testConfig
};