// 提供自动完成选项
export const PROVIDER_OPTIONS = [
    { value: 'aws', label: 'AWS S3' },
    { value: 'minio', label: 'MinIO' },
    { value: 'aliyun', label: '阿里云OSS' },
    // 可以添加更多选项
]

export const REGION_OPTIONS = [
    { value: 'cn-north-1', label: 'cn-north-1' },
    { value: 'us-east-1', label: 'us-east-1' },
    { value: 'cn-hangzhou', label: 'cn-hangzhou' },
    { value: 'cn-guangzhou', label: 'cn-guangzhou' },
    // 可以添加更多选项
]

// 定义文件类型枚举
export enum FileType {
    Image = 'image',
    Video = 'video',
    Text = 'text',
    Xlsx = 'xlsx',
    Ppt = 'ppt',
    Pdf = 'pdf',
    Zip = 'zip',
    Audio = 'audio',
    Other = 'other'
}