import {invoke} from "@tauri-apps/api/core";
import {listen, UnlistenFn} from "@tauri-apps/api/event";
import {FileList, TransferProgress} from "@/types";

// 获取文件列表
const listFile = async (
    id: string,
    path?: string,
    nextToken?: string
): Promise<FileList> => {
    path = path || '';
    if (path.startsWith("/")) {
        path = path.substring(1);
    }
    return await invoke<FileList>('file_list', {
        id,
        path: path,
        nextToken: nextToken || ''
    });
};

// 下载文件
const downloadFile = async (id: string, path: string): Promise<number[]> => {
    return await invoke<number[]>('file_download', {id, path});
};

// 删除文件或目录
const deleteFile = async (id: string, key: string): Promise<void> => {
    return await invoke<void>('file_delete', {id, key});
};

// 获取文件预览URL
const getFilePreviewUrl = async (id: string, key: string): Promise<string> => {
    return await invoke<string>('file_get_preview_url', {id, key});
};

// 上传文件/文件夹
const uploadFile = async (
    id: string,
    remotePath: string,
    localPath: string[]
): Promise<TransferProgress[]> => {
    return await invoke<TransferProgress[]>('file_upload', {id, remotePath, localPath});
};

const downloadFilePath = async (
    id: string,
    remoteKeys: string[],
    localPath: string
): Promise<TransferProgress[]> => {
    return await invoke<TransferProgress[]>('file_download_path', {id, remoteKeys, localPath});
};

// 监听传输进度事件
const listenTransferProgress = async (
    callback: (progress: TransferProgress) => void
): Promise<UnlistenFn> => {
    return await listen<TransferProgress>('transfer_process', (event) => {
        callback(event.payload);
    });
};

export const fileApi = {
    listFile,
    downloadFile,
    deleteFile,
    getFilePreviewUrl,
    uploadFile,
    listenTransferProgress,
    downloadFilePath
};
