<script setup lang="ts">
import {computed, h, onMounted, onUnmounted, reactive, ref} from 'vue'
import {useRoute} from 'vue-router'
import {message} from 'ant-design-vue'
import {
  AppstoreOutlined,
  MenuOutlined,
  ReloadOutlined,
  UnorderedListOutlined,
  UploadOutlined
} from '@ant-design/icons-vue'
import {FileItem, FileList, OssConfig} from '@/types'
import {open} from '@tauri-apps/plugin-dialog';
import router from "@/router";
import dayjs from "dayjs";
import {hideLoading, showLoading} from '@/utils/loading.ts'
import TransferIndicator from '@/views/components/TransferIndicator.vue'
import FilePreviewModal from '@/views/components/FilePreviewModal.vue';
import ContextMenu from '@/views/components/ContextMenu.vue'; // 导入新组件
import {configApi} from "@/services/config.ts";
import {fileApi} from "@/services/file.ts";
import SvgIcon from "@/components/Svg.vue";
import {formatFileSize, getFileType} from "@/utils/utils.ts";

const route = useRoute()

// 状态变量
const configList = ref<OssConfig[]>([])
const selectedConfig = ref<string>('')
const currentPathParts = ref<string[]>([])
const fileList = ref<FileItem[]>([])
const searchValue = ref<string>('')
const viewMode = ref<'list' | 'grid'>('list')
// 预览相关状态 - 简化
const previewVisible = ref<boolean>(false)
const previewFile = ref<FileItem | null>(null)
const transferRef = ref<InstanceType<typeof TransferIndicator> | null>(null)

const currentPath = computed(() => {
  return currentPathParts.value.join('/') + "/"
})
const getCompletePath = (file: FileItem): string => {
  let suffix = file.isDir ? "/" : "";
  let prefix = currentPathParts.value.length > 0 ? currentPath.value : "";
  return `${prefix}${file.name}${suffix}`
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

// 右键相关变量
const contextMenu = reactive({
  visible: false,
  x: 0,
  y: 0,
  file: null as FileItem | null
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

    if (selectedConfig.value) {
      await loadFileList()
    }
  } catch (error) {
    console.error('加载配置列表失败:', error)
    message.error('加载配置列表失败！')
  }
}

// 加载文件列表 - 支持分页
const loadFileList = async (append = false): Promise<void> => {
  if (!selectedConfig.value) return

  // 区分初始加载和上拉加载
  if (!append) {
    showLoading('加载中...')
    fileList.value = [] // 清空现有列表
    nextToken.value = null // 重置分页令牌
  } else {
    isLoadingMore.value = true
  }

  try {
    const result: FileList = await fileApi.listFile(
        selectedConfig.value,
        currentPath.value,
        append ? nextToken.value || '' : ''
    )

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
    console.error('加载文件列表失败:', error)
    message.error('加载文件列表失败！')
  } finally {
    if (!append) {
      hideLoading()
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
  await loadFileList(false) // 重新加载第一页
}

// 搜索文件
const onSearch = (e: KeyboardEvent) => {
  if (e.keyCode === 229) {
    console.log('composing')
    return
  }
  // TODO 搜索实现
}

// 导航到根目录 - 重置分页
const goToRoot = async (): Promise<void> => {
  currentPathParts.value = []
  await loadFileList(false)
}

// 导航到指定路径 - 重置分页
const navigateToPath = async (index: number): Promise<void> => {
  currentPathParts.value = currentPathParts.value.slice(0, index + 1)
  await loadFileList(false)
}

// 在 handleFileClick 方法中，可能需要处理目录路径
const handleFileClick = async (file: FileItem): Promise<void> => {
  if (file.isDir) {
    // 进入文件夹
    currentPathParts.value.push(file.name)
    await loadFileList(false) // 重置分页
  } else {
    // 根据文件类型决定是否预览
    const fileType = getFileType(file.name)
    if (['image', 'video', 'text'].includes(fileType)) {
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
  try {
    showLoading(`正在${file.isDir ? '递归删除目录' : '删除文件'}...`)
    await fileApi.deleteFile(
        selectedConfig.value,
        getCompletePath(file)
    )
    message.success('删除成功！')
    await loadFileList()
  } catch (error) {
    console.error('删除文件失败:', error)
    message.error('删除文件失败！')
  } finally {
    hideLoading()
  }
}


const selectFiles = async () => {
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
  await router.push({name: "FileManager", params: {id}})
  selectedConfig.value = id
  currentPathParts.value = []
  await loadFileList()
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
  console.log(contextMenu)
};

// 隐藏右键菜单 - 简化
const hideContextMenu = () => {
  contextMenu.visible = false;
};

// 在组件挂载后添加事件监听
onMounted(async () => {
  await loadConfigList()
  // 监听全局点击事件，点击其他地方时隐藏右键菜单
  window.addEventListener('click', hideContextMenu)
})

// 在组件卸载前移除事件监听
onUnmounted(() => {
  window.removeEventListener('click', hideContextMenu)
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
      </div>
      <div class="header-right button-group">
        <a-input class="search-input"
                 v-model:value="searchValue"
                 placeholder="搜索文件..."
                 style="width: 200px; margin-right: 12px;"
                 @keydown.enter.prevent="onSearch"
        />
        <a-flex gap="small">
          <a-button title="刷新当前目录" @click="refreshFiles" :icon="h(ReloadOutlined)"/>
          <a-button title="上传文件" type="primary" :icon="h(UploadOutlined)" @click="selectFiles"/>
          <a-button title="上传文件夹" type="primary" @click="selectFolders">
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
        <span class="file-count">共 {{ fileList.length }} 个对象</span>
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
            v-for="file in fileList"
            :key="file.name"
            class="file-item file-item-style"
            @dblclick="handleFileClick(file)"
            @contextmenu.prevent.stop="showContextMenu($event, file)"
        >
          <div class="file-name">
            <span class="file-icon">
              <svg-icon name="directory" v-if="file.isDir"/>
              <svg-icon :name="getFileType(file.name)" v-else/>
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
        @download="downloadFile"
        @delete="deleteFile"
    />

    <!-- 预览模态框 -->
    <FilePreviewModal
        v-model:visible="previewVisible"
        :file="previewFile"
        :config-id="selectedConfig"
        :current-path="currentPath"
        @close="closePreview"
        @download="downloadFile"
    />
    <!-- 进度条组件 -->
    <TransferIndicator ref="transferRef" :config_id="selectedConfig" :upload_path="currentPath"/>
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
</style>
