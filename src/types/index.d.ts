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

export interface TransferProgress {
    id: string
    config_id: string
    name: string
    from_path: string
    to_path: string
    size: number
    progress: number
    status: string
}