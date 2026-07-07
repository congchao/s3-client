// 定义前端表单类型（驼峰命名）
export interface OssConfig {
    id: string
    name: string
    provider: string
    region: string
    accessKey: string
    secretKey: string
    endpoint: string
    bucket: string
    pathStyle: string
    sort: number
}

export interface FileItem {
    name: string
    isDir: boolean
    size: number | null
    lastModified: string | null
    contentType: string | null
}

export interface FileList {
    nextToken: string | null
    objects: FileItem[]
}

export interface BucketInfo {
    name: string
    creationDate: string | null
}

export interface BucketPermissions {
    list: boolean
    read: boolean
    write: boolean
    delete: boolean
}

export interface ContextMenuSettings {
    download: boolean
    rename: boolean
    moveItem: boolean
    duplicate: boolean
    share: boolean
    delete: boolean
    copyPath: boolean
    parquetToExcel: boolean
}

export interface AppSettings {
    fileContextMenu: ContextMenuSettings
    directoryContextMenu: ContextMenuSettings
}

export interface AppUpdateCheckResult {
    currentVersion: string
    latestVersion: string
    latestTag: string
    releaseName: string | null
    releaseUrl: string | null
    publishedAt: string | null
    updateAvailable: boolean
    skipped: boolean
    shouldPrompt: boolean
}

export interface TransferProgress {
    id: string
    config_id: string
    bucket: string
    name: string
    from_path: string
    to_path: string
    size: number
    progress: number
    status: 'waiting' | 'uploading' | 'downloading' | 'completed' | 'failed' | 'cancelled'
}
