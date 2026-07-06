import {invoke} from "@tauri-apps/api/core";
import {listen, UnlistenFn} from "@tauri-apps/api/event";
import {BucketInfo, BucketPermissions, FileList, TransferProgress} from "@/types";

// 获取文件列表
const listFile = async (
    id: string,
    bucket: string,
    path?: string,
    nextToken?: string
): Promise<FileList> => {
    path = path || '';
    if (path.startsWith("/")) {
        path = path.substring(1);
    }
    return await invoke<FileList>('file_list', {
        id,
        bucket,
        path: path,
        nextToken: nextToken || ''
    });
};

// 下载文件
const downloadFile = async (id: string, bucket: string, path: string): Promise<number[]> => {
    return await invoke<number[]>('file_download', {id, bucket, path});
};

// 删除文件或目录
const deleteFile = async (id: string, bucket: string, key: string): Promise<void> => {
    return await invoke<void>('file_delete', {id, bucket, key});
};

// 获取文件预览URL
const getFilePreviewUrl = async (id: string, bucket: string, key: string): Promise<string> => {
    if (key.startsWith("/")) {
        key = key.substring(1);
    }
    return await invoke<string>('file_get_preview_url', {id, bucket, key});
};

// 上传文件/文件夹
const uploadFile = async (
    id: string,
    bucket: string,
    remotePath: string,
    localPath: string[]
): Promise<TransferProgress[]> => {
    return await invoke<TransferProgress[]>('file_upload', {id, bucket, remotePath, localPath});
};

const downloadFilePath = async (
    id: string,
    bucket: string,
    remoteKeys: string[],
    localPath: string
): Promise<TransferProgress[]> => {
    return await invoke<TransferProgress[]>('file_download_path', {id, bucket, remoteKeys, localPath});
};

const listBuckets = async (id: string): Promise<BucketInfo[]> => {
    return await invoke<BucketInfo[]>('bucket_list', {id});
};

const probePermissions = async (id: string, bucket: string): Promise<BucketPermissions> => {
    return await invoke<BucketPermissions>('bucket_probe_permissions', {id, bucket});
};

const createDirectory = async (id: string, bucket: string, key: string): Promise<void> => {
    return await invoke<void>('file_create_directory', {id, bucket, key});
};

const copyFile = async (
    id: string,
    bucket: string,
    sourceKey: string,
    targetKey: string
): Promise<void> => {
    return await invoke<void>('file_copy', {id, bucket, sourceKey, targetKey});
};

const moveFile = async (
    id: string,
    bucket: string,
    sourceKey: string,
    targetKey: string
): Promise<void> => {
    return await invoke<void>('file_move', {id, bucket, sourceKey, targetKey});
};

const createPresignedUrl = async (
    id: string,
    bucket: string,
    key: string,
    expiresSeconds: number
): Promise<string> => {
    return await invoke<string>('file_create_presigned_url', {id, bucket, key, expiresSeconds});
};

const cancelTransfer = async (taskId: string): Promise<void> => {
    return await invoke<void>('file_transfer_cancel', {taskId});
};

const retryTransfer = async (
    task: TransferProgress,
    transferType: 'upload' | 'download'
): Promise<TransferProgress> => {
    return await invoke<TransferProgress>('file_transfer_retry', {task, transferType});
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
    listBuckets,
    probePermissions,
    createDirectory,
    copyFile,
    moveFile,
    createPresignedUrl,
    cancelTransfer,
    retryTransfer,
    listenTransferProgress,
    downloadFilePath
};
