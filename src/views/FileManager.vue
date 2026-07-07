<script setup lang="ts">
import {computed, h, onMounted, onUnmounted, reactive, ref} from 'vue'
import {useRoute} from 'vue-router'
import {message} from 'ant-design-vue'
import {
  AppstoreOutlined,
  FolderAddOutlined,
  LinkOutlined,
  MenuOutlined,
  ReloadOutlined,
  UnorderedListOutlined,
  UploadOutlined
} from '@ant-design/icons-vue'
import {AppSettings, BucketPermissions, ContextMenuSettings, FileItem, FileList, OssConfig} from '@/types'
import {open, save} from '@tauri-apps/plugin-dialog';
import {readText} from '@tauri-apps/plugin-clipboard-manager';
import router from "@/router";
import dayjs from "dayjs";
import {showLoading} from '@/utils/loading.ts'
import TransferIndicator, {type UploadCompletedPayload} from '@/views/components/TransferIndicator.vue'
import FilePreviewModal from '@/views/components/FilePreviewModal.vue';
import ContextMenu from '@/views/components/ContextMenu.vue'; // 导入新组件
import {configApi} from "@/services/config.ts";
import {fileApi} from "@/services/file.ts";
import SvgIcon from "@/components/Svg.vue";
import {formatFileSize, getFileIconType, getFileType} from "@/utils/utils.ts";
import {FileType} from "@/types/constants.ts";

const route = useRoute()

// 状态变量
const configList = ref<OssConfig[]>([])
const selectedConfig = ref<string>('')
const selectedBucket = ref<string>('')
const currentPathParts = ref<string[]>([])
const fileList = ref<FileItem[]>([])
const searchValue = ref<string>('')
const isComposing = ref<boolean>(false)
const viewMode = ref<'list' | 'grid'>('list')
// 预览相关状态 - 简化
const previewVisible = ref<boolean>(false)
const previewFile = ref<FileItem | null>(null)
const transferRef = ref<InstanceType<typeof TransferIndicator> | null>(null)
const permissions = ref<BucketPermissions | null>(null)

type ObjectOperationMode = 'mkdir' | 'rename' | 'move' | 'copy'

const objectOperation = reactive({
  visible: false,
  loading: false,
  mode: 'mkdir' as ObjectOperationMode,
  title: '',
  label: '',
  value: '',
  file: null as FileItem | null,
})

const shareModal = reactive({
  visible: false,
  loading: false,
  expiresSeconds: 3600,
  url: '',
  file: null as FileItem | null,
})

const defaultMenuSettings = (): ContextMenuSettings => ({
  download: true,
  rename: true,
  moveItem: true,
  duplicate: true,
  share: true,
  delete: true,
  copyPath: true,
  parquetToExcel: true,
})

const appSettings = reactive<AppSettings>({
  fileContextMenu: defaultMenuSettings(),
  directoryContextMenu: defaultMenuSettings(),
})

const currentPath = computed(() => {
  return currentPathParts.value.length > 0 ? `${currentPathParts.value.join('/')}/` : ''
})
const resetSearch = () => {
  searchValue.value = ''
}
// 前端搜索过滤（仅当前目录列表）
const filteredFileList = computed(() => {
  const keyword = searchValue.value.trim().toLowerCase()
  if (!keyword) return fileList.value
  return fileList.value.filter((file) =>
      file.name.toLowerCase().includes(keyword)
  )
})
const getCompletePath = (file: FileItem): string => {
  let suffix = file.isDir ? "/" : "";
  let prefix = currentPathParts.value.length > 0 ? currentPath.value : "";
  return `${prefix}${file.name}${suffix}`
}

const canWrite = computed(() => permissions.value?.write !== false)
const canDelete = computed(() => permissions.value?.delete !== false)
const canRead = computed(() => permissions.value?.read !== false)

const copyText = async (text: string): Promise<void> => {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text)
  } else {
    const textarea = document.createElement('textarea')
    textarea.value = text
    textarea.style.position = 'fixed'
    textarea.style.opacity = '0'
    document.body.appendChild(textarea)
    textarea.select()
    document.execCommand('copy')
    document.body.removeChild(textarea)
  }
}

const joinRemotePath = (basePath: string, name: string): string => {
  const base = basePath.trim().replace(/^\/+|\/+$/g, '')
  const normalizedName = name.trim().replace(/^\/+|\/+$/g, '')
  if (!base) return normalizedName
  if (!normalizedName) return `${base}/`
  return `${base}/${normalizedName}`
}

const withDirectorySuffix = (key: string, isDir: boolean): string => {
  if (!isDir) return key.replace(/^\/+/, '')
  const normalized = key.replace(/^\/+|\/+$/g, '')
  return normalized ? `${normalized}/` : ''
}

const loadAppSettings = async (): Promise<void> => {
  try {
    const result = await configApi.getSettings()
    Object.assign(appSettings.fileContextMenu, result.fileContextMenu)
    Object.assign(appSettings.directoryContextMenu, result.directoryContextMenu)
  } catch (error) {
    console.error('加载设置失败:', error)
  }
}

// 预览文件 - 简化
const previewFileContent = async (file: FileItem): Promise<void> => {
  if (file.isDir) return
  previewFile.value = file
  previewVisible.value = true
}

// 关闭预览 - 简化
const closePreview = (): void => {
  previewVisible.value = false
  previewFile.value = null
}

// 分页相关状态
const nextToken = ref<string | null>(null)
const hasMore = ref<boolean>(false)
const isLoadingMore = ref<boolean>(false)
let fileListLoadVersion = 0

// 右键相关变量
const contextMenu = reactive({
  visible: false,
  x: 0,
  y: 0,
  file: null as FileItem | null
})

const currentContextMenuSettings = computed(() => {
  return contextMenu.file?.isDir ? appSettings.directoryContextMenu : appSettings.fileContextMenu
})

// 加载配置列表
const loadConfigList = async (): Promise<void> => {
  try {
    configList.value = await configApi.getConfig()

    // 如果路由参数中有配置名称，使用它
    if (route.params.id) {
      selectedConfig.value = route.params.id as string
    } else if (configList.value.length > 0) {
      selectedConfig.value = configList.value[0].id
    }

    if (route.params.bucket) {
      selectedBucket.value = route.params.bucket as string
    } else {
      selectedBucket.value = configList.value.find(c => c.id === selectedConfig.value)?.bucket || ''
    }

    if (selectedConfig.value && !selectedBucket.value) {
      await router.push({name: 'BucketList', params: {id: selectedConfig.value}})
      return
    }

    if (selectedConfig.value && selectedBucket.value) {
      await loadPermissions()
      await loadFileList()
    }
  } catch (error) {
    console.error('加载配置列表失败:', error)
    message.error('加载配置列表失败！')
  }
}

const loadPermissions = async (): Promise<void> => {
  if (!selectedConfig.value || !selectedBucket.value) return
  try {
    permissions.value = await fileApi.probePermissions(selectedConfig.value, selectedBucket.value)
  } catch (error) {
    console.error('权限探测失败:', error)
    permissions.value = null
  }
}

// 加载文件列表 - 支持分页
const loadFileList = async (append = false): Promise<void> => {
  if (!selectedConfig.value || !selectedBucket.value) return
  const requestVersion = ++fileListLoadVersion
  let loadingControl: ReturnType<typeof showLoading> | null = null

  // 区分初始加载和上拉加载
  if (!append) {
    loadingControl = showLoading('加载中...', 'large', {
      cancelText: '取消加载',
      onCancel: () => {
        fileListLoadVersion++
        message.info('已取消当前加载')
      }
    })
    fileList.value = [] // 清空现有列表
    nextToken.value = null // 重置分页令牌
  } else {
    isLoadingMore.value = true
  }

  try {
    const result: FileList = await fileApi.listFile(
        selectedConfig.value,
        selectedBucket.value,
        currentPath.value,
        append ? nextToken.value || '' : ''
    )

    if (requestVersion !== fileListLoadVersion || loadingControl?.cancelled) {
      return
    }

    // 更新分页信息
    nextToken.value = result.nextToken
    hasMore.value = !!result.nextToken

    // 根据加载类型决定是否追加数据
    if (append) {
      fileList.value = [...fileList.value, ...result.objects]
      fileList.value.sort((a: FileItem, b: FileItem) => {
        if (a.isDir && b.isDir) {
          return a.name.localeCompare(b.name)
        } else if (a.isDir) {
          return -1
        } else if (b.isDir) {
          return 1
        }
        return 0
      })
    } else {
      fileList.value = result.objects
    }
  } catch (error) {
    if (requestVersion !== fileListLoadVersion || loadingControl?.cancelled) {
      return
    }
    console.error('加载文件列表失败:', error)
    message.error('加载文件列表失败！')
  } finally {
    if (!append) {
      loadingControl?.close()
    } else {
      isLoadingMore.value = false
    }
  }
}

// 格式化日期
const formatDate = (dateString: string): string => {
  return dayjs(dateString).format('YYYY-MM-DD HH:mm:ss')
}

// 切换视图模式
const changeViewMode = (mode: 'list' | 'grid'): void => {
  viewMode.value = mode
}

// 刷新文件列表 - 重置分页
const refreshFiles = async (): Promise<void> => {
  await loadPermissions()
  await loadFileList(false) // 重新加载第一页
}

const normalizeRemotePath = (path: string): string => {
  const normalized = path.trim().replace(/^\/+|\/+$/g, '')
  return normalized ? `${normalized}/` : ''
}

const getParentRemotePath = (path: string): string => {
  const normalized = normalizeRemotePath(path).replace(/\/$/, '')
  const lastSlashIndex = normalized.lastIndexOf('/')
  return lastSlashIndex >= 0 ? `${normalized.slice(0, lastSlashIndex)}/` : ''
}

const handleUploadCompleted = async ({configId, bucket, uploadPath}: UploadCompletedPayload): Promise<void> => {
  if (configId !== selectedConfig.value) return
  if (bucket !== selectedBucket.value) return

  const current = normalizeRemotePath(currentPath.value)
  const uploaded = normalizeRemotePath(uploadPath)
  const uploadedParent = getParentRemotePath(uploaded)

  if (current === uploaded || current === uploadedParent) {
    await refreshFiles()
  }
}

// 搜索文件
const onSearch = (e: KeyboardEvent) => {
  if (e.keyCode === 229 || isComposing.value) {
    return
  }
}

const pasteTextIntoSearch = (text: string, target?: EventTarget | null): void => {
  if (!text) return
  const input = target instanceof HTMLInputElement ? target : null
  if (!input) {
    searchValue.value = `${searchValue.value}${text}`
    return
  }

  const start = input.selectionStart ?? searchValue.value.length
  const end = input.selectionEnd ?? searchValue.value.length
  searchValue.value = `${searchValue.value.slice(0, start)}${text}${searchValue.value.slice(end)}`
  requestAnimationFrame(() => {
    const cursor = start + text.length
    input.setSelectionRange(cursor, cursor)
  })
}

const handleSearchPasteShortcut = async (event: KeyboardEvent): Promise<void> => {
  if (!((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'v')) return

  event.preventDefault()
  try {
    const text = await readText()
    if (!text) return
    pasteTextIntoSearch(text, event.target)
  } catch (error) {
    console.error('读取剪贴板失败:', error)
    message.warning('读取剪贴板失败')
  }
}

// 导航到根目录 - 重置分页
const goToRoot = async (): Promise<void> => {
  currentPathParts.value = []
  resetSearch()
  await loadFileList(false)
}

// 导航到指定路径 - 重置分页
const navigateToPath = async (index: number): Promise<void> => {
  currentPathParts.value = currentPathParts.value.slice(0, index + 1)
  resetSearch()
  await loadFileList(false)
}

// 在 handleFileClick 方法中，可能需要处理目录路径
const handleFileClick = async (file: FileItem): Promise<void> => {
  if (file.isDir) {
    // 进入文件夹
    currentPathParts.value.push(file.name)
    resetSearch()
    await loadFileList(false) // 重置分页
  } else {
    // 根据文件类型决定是否预览
    const fileType = getFileType(file.name)
    if ([FileType.Image, FileType.Video, FileType.Text, FileType.Csv, FileType.Json, FileType.Markdown, FileType.Parquet].includes(fileType)) {
      await previewFileContent(file)
    } else {
      // 其他类型直接下载
      await downloadFile(file)
    }
  }
}

const downloadFile = async (file: FileItem): Promise<void> => {
  try {
    const local_path = await open({
      multiple: false,
      directory: true
    });
    if (local_path != null) {
      transferRef.value?.download([getCompletePath(file)], local_path)
    }
  } catch (error) {
    console.error('下载文件失败:', error);
    message.error('下载文件失败！');
  }
};

// 删除文件
const deleteFile = async (file: FileItem): Promise<void> => {
  if (!canDelete.value) {
    message.warning('当前账号没有删除权限')
    return
  }
  let cancelled = false
  const loadingControl = showLoading(`正在${file.isDir ? '递归删除目录' : '删除文件'}...`, 'large', {
    cancelText: '取消等待',
    onCancel: () => {
      cancelled = true
      message.info('已取消等待')
    }
  })
  try {
    await fileApi.deleteFile(
        selectedConfig.value,
        selectedBucket.value,
        getCompletePath(file)
    )
    if (cancelled) return
    message.success('删除成功！')
    await loadFileList()
  } catch (error) {
    if (cancelled) return
    console.error('删除文件失败:', error)
    message.error('删除文件失败！')
  } finally {
    loadingControl.close()
  }
}

const copyFilePath = async (file: FileItem): Promise<void> => {
  try {
    const keyPath = getCompletePath(file)
    const fullPath = selectedBucket.value ? `${selectedBucket.value}/${keyPath}` : keyPath
    await copyText(fullPath)
    message.success('已复制完整路径')
  } catch (error) {
    console.error('复制路径失败:', error)
    message.error('复制路径失败')
  }
}

const ensureWritePermission = (): boolean => {
  if (!canWrite.value) {
    message.warning('当前账号没有写入权限')
    return false
  }
  return true
}

const openObjectOperation = (mode: ObjectOperationMode, file: FileItem | null = null): void => {
  if (!ensureWritePermission()) return

  objectOperation.mode = mode
  objectOperation.file = file
  objectOperation.loading = false

  if (mode === 'mkdir') {
    objectOperation.title = '新建文件夹'
    objectOperation.label = '文件夹名称'
    objectOperation.value = ''
  } else if (mode === 'rename' && file) {
    objectOperation.title = '重命名'
    objectOperation.label = '新名称'
    objectOperation.value = file.name
  } else if (mode === 'move' && file) {
    objectOperation.title = '移动'
    objectOperation.label = '目标路径'
    objectOperation.value = getCompletePath(file)
  } else if (mode === 'copy' && file) {
    objectOperation.title = '复制'
    objectOperation.label = '目标路径'
    const sourceKey = getCompletePath(file)
    objectOperation.value = file.isDir
        ? sourceKey.replace(/\/$/, '-copy/')
        : sourceKey.replace(/([^/]+)$/, 'copy-$1')
  }

  objectOperation.visible = true
}

const submitObjectOperation = async (): Promise<void> => {
  const value = objectOperation.value.trim()
  if (!value) {
    message.warning('请输入有效路径')
    return
  }

  if (objectOperation.mode === 'rename' && value.includes('/')) {
    message.warning('重命名只支持输入名称，不支持路径')
    return
  }

  objectOperation.loading = true
  try {
    if (objectOperation.mode === 'mkdir') {
      const directoryKey = withDirectorySuffix(joinRemotePath(currentPath.value, value), true)
      await fileApi.createDirectory(selectedConfig.value, selectedBucket.value, directoryKey)
      message.success('文件夹创建成功')
    } else if (objectOperation.file) {
      const sourceKey = getCompletePath(objectOperation.file)
      let targetKey = value
      if (objectOperation.mode === 'rename') {
        targetKey = joinRemotePath(currentPath.value, value)
      }
      targetKey = withDirectorySuffix(targetKey, objectOperation.file.isDir)

      if (objectOperation.mode === 'rename' || objectOperation.mode === 'move') {
        await fileApi.moveFile(selectedConfig.value, selectedBucket.value, sourceKey, targetKey)
        message.success(objectOperation.mode === 'rename' ? '重命名成功' : '移动成功')
      } else if (objectOperation.mode === 'copy') {
        await fileApi.copyFile(selectedConfig.value, selectedBucket.value, sourceKey, targetKey)
        message.success('复制成功')
      }
    }

    objectOperation.visible = false
    await refreshFiles()
  } catch (error) {
    console.error('对象操作失败:', error)
    message.error(`操作失败: ${error instanceof Error ? error.message : String(error)}`)
  } finally {
    objectOperation.loading = false
  }
}

const openShareModal = (file: FileItem): void => {
  if (file.isDir) return
  if (!canRead.value) {
    message.warning('当前账号没有读取权限')
    return
  }
  shareModal.file = file
  shareModal.expiresSeconds = 3600
  shareModal.url = ''
  shareModal.visible = true
}

const generatePresignedUrl = async (): Promise<void> => {
  if (!shareModal.file) return
  shareModal.loading = true
  try {
    shareModal.url = await fileApi.createPresignedUrl(
        selectedConfig.value,
        selectedBucket.value,
        getCompletePath(shareModal.file),
        shareModal.expiresSeconds
    )
  } catch (error) {
    console.error('生成预签名链接失败:', error)
    message.error('生成预签名链接失败')
  } finally {
    shareModal.loading = false
  }
}

const copyShareUrl = async (): Promise<void> => {
  if (!shareModal.url) return
  await copyText(shareModal.url)
  message.success('链接已复制')
}

const exportParquetToExcel = async (file: FileItem): Promise<void> => {
  if (!file.isDir && !file.name.toLowerCase().endsWith('.parquet')) return
  if (!canRead.value) {
    message.warning('当前账号没有读取权限')
    return
  }

  const defaultExcelName = file.isDir
      ? `${file.name.replace(/\/$/g, '') || 'parquet-folder'}.xlsx`
      : file.name.replace(/\.parquet$/i, '.xlsx')
  const outputPath = await save({
    defaultPath: defaultExcelName,
    filters: [{name: 'Excel 工作簿', extensions: ['xlsx']}],
  })
  if (!outputPath) return

  await transferRef.value?.exportParquetXlsx(getCompletePath(file), outputPath)
}

const selectFiles = async () => {
  if (!ensureWritePermission()) return
  // 调用原生对话框，核心配置：同时开启 文件+文件夹 选择
  const res = await open({
    multiple: true,
    directory: false
  });

  // 处理选中结果：统一转为数组，无选中则返回空数组
  const selectedPaths: string[] = [];
  if (typeof res === 'string') {
    selectedPaths.push(res); // 单选时返回字符串，转成数组
  } else if (Array.isArray(res)) {
    selectedPaths.push(...res); // 多选时直接返回数组
  }
  if (selectedPaths.length > 0) {
    await transferRef.value?.upload(selectedPaths)
  }
}

const selectFolders = async () => {
  if (!ensureWritePermission()) return
  // 调用原生对话框，核心配置：同时开启 文件+文件夹 选择
  const res = await open({
    multiple: true,
    directory: true
  });

  // 处理选中结果：统一转为数组，无选中则返回空数组
  const selectedPaths: string[] = [];
  if (typeof res === 'string') {
    selectedPaths.push(res); // 单选时返回字符串，转成数组
  } else if (Array.isArray(res)) {
    selectedPaths.push(...res); // 多选时直接返回数组
  }
  if (selectedPaths.length > 0) {
    await transferRef.value?.upload(selectedPaths)
  }
}


// 更新onConfigChange方法，确保在配置改变时重置路径
const onConfigChange = async (id: string): Promise<void> => {
  selectedConfig.value = id
  const bucket = configList.value.find(c => c.id === id)?.bucket || ''
  selectedBucket.value = bucket
  if (!bucket) {
    await router.push({name: 'BucketList', params: {id}})
    return
  }
  await router.push({name: "FileManager", params: {id, bucket}})
  currentPathParts.value = []
  resetSearch()
  await loadPermissions()
  await loadFileList()
}

const goToBucketList = async (): Promise<void> => {
  if (!selectedConfig.value) return
  await router.push({name: 'BucketList', params: {id: selectedConfig.value}})
}

// 添加返回配置列表的方法
const goToConfigList = async () => {
  await router.push({name: "ConfigList"})
}

// 滚动事件处理，实现上拉加载
const handleScroll = (event: Event) => {
  const target = event.target as HTMLElement
  const scrollTop = target.scrollTop
  const scrollHeight = target.scrollHeight
  const clientHeight = target.clientHeight

  // 判断是否滚动到底部
  if (scrollHeight - scrollTop <= clientHeight + 10 && hasMore.value && !isLoadingMore.value) {
    // 防止重复触发
    if (!isLoadingMore.value && nextToken.value) {
      loadFileList(true) // 传入true表示追加加载
    }
  }
}

// 显示右键菜单 - 简化
const showContextMenu = (event: MouseEvent, file: FileItem | null) => {
  event.preventDefault() // 阻止默认右键菜单
  contextMenu.file = file;
  contextMenu.visible = true;
  contextMenu.x = event.clientX;
  contextMenu.y = event.clientY;
};

// 隐藏右键菜单 - 简化
const hideContextMenu = () => {
  contextMenu.visible = false;
};

const handleAppSettingsSaved = (): void => {
  loadAppSettings()
}

// 在组件挂载后添加事件监听
onMounted(async () => {
  await loadAppSettings()
  await loadConfigList()
  // 监听全局点击事件，点击其他地方时隐藏右键菜单
  window.addEventListener('click', hideContextMenu)
  window.addEventListener('app-settings-saved', handleAppSettingsSaved)
})

// 在组件卸载前移除事件监听
onUnmounted(() => {
  window.removeEventListener('click', hideContextMenu)
  window.removeEventListener('app-settings-saved', handleAppSettingsSaved)
})


</script>


<template>
  <div class="oss-file-manager page-layout">
    <!-- 顶部工具栏 -->
    <a-layout-header class="header header-layout">
      <div class="header-left">
        <a-button
            @click="goToConfigList" style="margin-right: 16px;"
            :icon="h(MenuOutlined)"
            title="返回配置列表"
        >
        </a-button>
        <a-select
            v-model:value="selectedConfig"
            placeholder="选择OSS配置" style="width: 100px; margin-right: 16px;"
            @change="onConfigChange"
        >
          <a-select-option
              v-for="config in configList"
              :key="config.name"
              :value="config.id"
          >
            {{ config.name }}
          </a-select-option>
        </a-select>
        <a-button v-if="selectedBucket" @click="goToBucketList" title="切换存储桶">
          {{ selectedBucket }}
        </a-button>
        <a-space v-if="permissions" class="permission-tags">
          <a-tag :color="permissions.list ? 'green' : 'red'">列出</a-tag>
          <a-tag :color="permissions.read ? 'green' : 'red'">读取</a-tag>
          <a-tag :color="permissions.write ? 'green' : 'red'">写入</a-tag>
          <a-tag :color="permissions.delete ? 'green' : 'red'">删除</a-tag>
        </a-space>
      </div>
      <div class="header-right button-group">
        <a-input class="search-input"
                 v-model:value="searchValue"
                 placeholder="搜索文件..."
                 style="width: 200px; margin-right: 12px;"
                 @keydown.enter.prevent="onSearch"
                 @keydown="handleSearchPasteShortcut"
                 @compositionstart="isComposing = true"
                 @compositionend="isComposing = false"
        />
        <a-flex gap="small">
          <a-button title="刷新当前目录" @click="refreshFiles" :icon="h(ReloadOutlined)"/>
          <a-button title="新建文件夹" :disabled="!canWrite" :icon="h(FolderAddOutlined)" @click="openObjectOperation('mkdir')"/>
          <a-button title="上传文件" type="primary" :disabled="!canWrite" :icon="h(UploadOutlined)" @click="selectFiles"/>
          <a-button title="上传文件夹" type="primary" :disabled="!canWrite" @click="selectFolders">
            <template #icon>
              <svg-icon name="upload_directory" style="width: 20px; height: 20px;"/>
            </template>
          </a-button>
        </a-flex>
      </div>
    </a-layout-header>

    <!-- 导航条 -->
    <div class="breadcrumb-bar">
      <div class="breadcrumb-left">
        <a-breadcrumb separator=">">
          <a-breadcrumb-item @click="goToRoot" style="cursor: pointer;">
            <span>根目录</span>
          </a-breadcrumb-item>
          <a-breadcrumb-item
              v-for="(path, index) in currentPathParts"
              :key="index"
              @click="navigateToPath(index)" style="cursor: pointer;"
          >
            <span>{{ path }}</span>
          </a-breadcrumb-item>
        </a-breadcrumb>
      </div>
      <div class="breadcrumb-right">
        <span class="file-count">共 {{ filteredFileList.length }} 个对象</span>
        <a-button-group>
          <a-button
              :type="viewMode === 'list' ? 'primary' : 'default'"
              @click="changeViewMode('list')"
              :icon="h(UnorderedListOutlined)"
          />
          <a-button
              :type="viewMode === 'grid' ? 'primary' : 'default'"
              @click="changeViewMode('grid')"
              :icon="h(AppstoreOutlined)"
          />
        </a-button-group>
      </div>
    </div>

    <!-- 文件列表显示区域 -->
    <a-layout-content class="content" @scroll="handleScroll">
      <div class="file-list-container file-list-container-style">
        <div
            v-for="file in filteredFileList"
            :key="file.name"
            class="file-item file-item-style"
            @dblclick="handleFileClick(file)"
            @contextmenu.prevent.stop="showContextMenu($event, file)"
        >
          <div class="file-name">
            <span class="file-icon">
              <svg-icon name="directory" v-if="file.isDir"/>
              <svg-icon :name="getFileIconType(file.name)" v-else/>
            </span>
            <span class="file-text">{{ file.name }}</span>
          </div>
          <div class="file-size">{{ file.isDir ? '' : formatFileSize(file.size || 0) }}</div>
          <div class="file-modified">{{ file.isDir ? '' : formatDate(file.lastModified || '') }}</div>
        </div>
        <!-- 加载更多指示器 -->
        <div v-if="isLoadingMore" class="loading-more loading-more-style">
          <a-spin size="small"/>
          <span style="margin-left: 8px;">loading...</span>
        </div>
      </div>
    </a-layout-content>

    <!-- 右键菜单 -->
    <ContextMenu
        v-model:visible="contextMenu.visible"
        :x="contextMenu.x"
        :y="contextMenu.y"
        :file="contextMenu.file"
        :settings="currentContextMenuSettings"
        @download="downloadFile"
        @delete="deleteFile"
        @copy="copyFilePath"
        @rename="(file) => openObjectOperation('rename', file)"
        @move="(file) => openObjectOperation('move', file)"
        @duplicate="(file) => openObjectOperation('copy', file)"
        @share="openShareModal"
        @parquet-to-excel="exportParquetToExcel"
    />

    <a-modal
        v-model:open="objectOperation.visible"
        :title="objectOperation.title"
        :confirm-loading="objectOperation.loading"
        ok-text="确定"
        cancel-text="取消"
        @ok="submitObjectOperation"
    >
      <a-form layout="vertical">
        <a-form-item :label="objectOperation.label">
          <a-input
              v-model:value="objectOperation.value"
              :placeholder="objectOperation.mode === 'rename' ? '请输入新名称' : '请输入目标路径'"
              @pressEnter="submitObjectOperation"
          />
        </a-form-item>
      </a-form>
    </a-modal>

    <a-modal
        v-model:open="shareModal.visible"
        title="预签名链接"
        :footer="null"
    >
      <a-form layout="vertical">
        <a-form-item label="有效期（秒）">
          <a-input-number
              v-model:value="shareModal.expiresSeconds"
              :min="1"
              :max="604800"
              style="width: 100%;"
          />
        </a-form-item>
        <a-form-item>
          <a-button type="primary" :loading="shareModal.loading" :icon="h(LinkOutlined)" @click="generatePresignedUrl">
            生成链接
          </a-button>
        </a-form-item>
        <a-form-item v-if="shareModal.url" label="链接">
          <a-textarea :value="shareModal.url" :auto-size="{ minRows: 3, maxRows: 5 }" readonly/>
        </a-form-item>
        <a-flex v-if="shareModal.url" justify="end">
          <a-button type="primary" @click="copyShareUrl">复制链接</a-button>
        </a-flex>
      </a-form>
    </a-modal>

    <!-- 预览模态框 -->
    <FilePreviewModal
        v-model:visible="previewVisible"
        :file="previewFile"
        :config-id="selectedConfig"
        :bucket="selectedBucket"
        :current-path="currentPath"
        @close="closePreview"
        @download="downloadFile"
        @parquet-to-excel="exportParquetToExcel"
    />
    <!-- 进度条组件 -->
    <TransferIndicator
        ref="transferRef"
        :config_id="selectedConfig"
        :bucket="selectedBucket"
        :upload_path="currentPath"
        @upload-completed="handleUploadCompleted"
    />
  </div>
</template>


<style scoped lang="less">
.breadcrumb-bar {
  display: flex;
  justify-content: space-between;
  padding: 4px 16px;
  background: #fafafa;
  border-bottom: 1px solid #e8e8e8;
  align-items: center;
}

.breadcrumb-left {
  flex: 1;
}

.breadcrumb-right {
  display: flex;
  align-items: center;
  gap: 16px;
}

.file-count {
  color: #888;
  margin-right: 16px;
}

.content {
  flex: 1;
  overflow: auto;
  background: #fff;
  padding: 4px 16px;
}

.permission-tags {
  margin-left: 12px;
}
</style>
