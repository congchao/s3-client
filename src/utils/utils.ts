// 重构后的文件类型判断函数
import {FileType} from "@/types/constants.ts";

export const getFileType = (fileName: string): FileType => {
    const ext = fileName.split('.').pop()?.toLowerCase() || ''

    // 定义各类型对应的扩展名集合
    const fileExtensions: Record<FileType, string[]> = {
        [FileType.Image]: ['jpg', 'jpeg', 'png', 'gif', 'bmp', 'webp', 'svg'],
        [FileType.Video]: ['mp4', 'avi', 'mov', 'wmv', 'flv', 'webm', 'm4v'],
        [FileType.Text]: ['txt', 'md', 'json', 'xml', 'html', 'css', 'js', 'ts', 'vue', 'py', 'java', 'cpp', 'c', 'h', 'sql', 'log', 'yaml', 'yml'],
        [FileType.Parquet]: ['parquet'],
        [FileType.Xlsx]: ['xlsx', 'xls'],
        [FileType.Ppt]: ['pptx', 'ppt'],
        [FileType.Pdf]: ['pdf'],
        [FileType.Zip]: ['zip', '7z', 'tar', 'gz', 'rar'],
        [FileType.Audio]: ['mp3'],
        [FileType.Other]: [] // 占位，实际不会匹配到
    }

    // 遍历类型查找匹配项
    for (const [fileType, extensions] of Object.entries(fileExtensions)) {
        if (extensions.includes(ext)) {
            return fileType as FileType
        }
    }

    return FileType.Other
}

// 图标类型（避免缺失 icon）
export const getFileIconType = (fileName: string): FileType => {
    return getFileType(fileName)
}

// 格式化文件大小
export const formatFileSize = (bytes: number): string => {
    if (bytes === 0) return '0 B'
    const k = 1024
    const sizes = ['B', 'KB', 'MB', 'GB']
    const i = Math.floor(Math.log(bytes) / Math.log(k))
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i]
}
