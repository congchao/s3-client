<template>
  <div>
    <!-- 进度条缩略信息 -->
    <div class="progress-summary" @click="toggleProgressDetail">
      <div class="progress-item">
        <UploadOutlined/>
        <span>{{ summary.completedUploads }}/{{ summary.totalUploads }}</span>
      </div>
      <div class="progress-item">
        <DownloadOutlined/>
        <span>{{ summary.completedDownloads }}/{{ summary.totalDownloads }}</span>
      </div>
    </div>

    <!-- 进度详情弹窗 -->
    <div v-if="showProgressDetail" class="progress-detail-panel" @click.stop>
      <div class="progress-tabs">
        <div
            :class="['tab', activeTab === 'upload' ? 'active' : '']"
            @click="switchTab('upload')"
        >
          上传 ({{ summary.completedUploads }}/{{ summary.totalUploads }})
        </div>
        <div
            :class="['tab', activeTab === 'download' ? 'active' : '']"
            @click="switchTab('download')"
        >
          下载 ({{ summary.completedDownloads }}/{{ summary.totalDownloads }})
        </div>
      </div>

      <div class="progress-list">
        <div
            v-for="item in filteredItems"
            :key="item.id"
            class="progress-item-detail"
        >
          <div class="progress-info">
            <div class="file-name">{{ item.name }}</div>
            <div class="file-path">{{ item.from_path }} → {{ item.to_path }}</div>
          </div>
          <div class="progress-status">
            <div class="progress-bar">
              <div
                  class="progress-fill"
                  :style="{ width: item.progress + '%' }"
              ></div>
            </div>
            <div class="progress-percent">{{ Math.round(item.progress) }}%</div>
            <div class="status-badge" :class="item.status">{{ item.status }}</div>
          </div>
        </div>
      </div>
    </div>

    <!-- 遮罩层 -->
    <div v-if="showProgressDetail" class="progress-overlay" @click="closeProgressDetail"></div>
  </div>
</template>

<script setup lang="ts">
import {computed, onMounted, onUnmounted, ref, watch} from 'vue'
import {DownloadOutlined, UploadOutlined} from '@ant-design/icons-vue'
import {listen, UnlistenFn} from "@tauri-apps/api/event"
import {TransferProgress} from "@/types";
import {getCurrentWebviewWindow} from "@tauri-apps/api/webviewWindow";
import {fileApi} from "@/services/file.ts";
import {message} from "ant-design-vue";

// 定义 props
interface Props {
  config_id: string,
  upload_path: string
}

const props = withDefaults(defineProps<Props>(), {
  config_id: "",
  upload_path: "",
})

// 状态变量
const showProgressDetail = ref<boolean>(false)
const activeTab = ref<string>('upload')
const uploadItems = ref<TransferProgress[]>([])
const downloadItems = ref<TransferProgress[]>([])
const uploadNotified = ref<boolean>(false)
const downloadNotified = ref<boolean>(false)
let transferEvent: UnlistenFn | null = null
let dragDropEvent: UnlistenFn | null = null


// 计算摘要信息
const summary = computed(() => {
  return {
    totalUploads: uploadItems.value.length,
    completedUploads: uploadItems.value.filter(item => item.progress >= 100).length,
    totalDownloads: downloadItems.value.length,
    completedDownloads: downloadItems.value.filter(item => item.progress >= 100).length
  }
})

const uploadAllCompleted = computed(() => {
  return uploadItems.value.length > 0 && uploadItems.value.every(item => item.status === 'completed')
})

const downloadAllCompleted = computed(() => {
  return downloadItems.value.length > 0 && downloadItems.value.every(item => item.status === 'completed')
})

// 根据当前标签页过滤项目
const filteredItems = computed(() => {
  if (activeTab.value === 'upload') {
    return uploadItems.value;
  } else {
    return downloadItems.value;
  }
})

// 事件处理函数
const toggleProgressDetail = () => {
  showProgressDetail.value = !showProgressDetail.value
}

const closeProgressDetail = () => {
  showProgressDetail.value = false
}

const switchTab = (tab: string) => {
  activeTab.value = tab
}

const upload = async (local_paths: string[]) => {
  try {
    const result: TransferProgress[] = await fileApi.uploadFile(
        props.config_id,
        props.upload_path,
        local_paths
    );
    uploadItems.value.push(...result)
  } catch (error) {
    console.error('上传启动失败:', error);
    message.error('上传启动失败');
  }
}

const download = async (remoteKeys: string[], localPath: string) => {
  try {
    console.log(remoteKeys,localPath)
    const result: TransferProgress[] = await fileApi.downloadFilePath(
        props.config_id,
        remoteKeys,
        localPath
    );
    console.log(result)
    downloadItems.value.push(...result)
  } catch (error) {
    console.error('下载启动失败:', error);
    message.error('下载启动失败');
  }
}

// 如果需要在组件内部监听事件，可以在这里实现
onMounted(async () => {
  transferEvent = await listen<TransferProgress>('transfer_process', (event) => {
    const progress: TransferProgress = event.payload;
    // 更新进度信息
    let index = uploadItems.value.findIndex(item => item.id === progress.id)
    if (index >= 0) {
      uploadItems.value[index] = progress
      console.log(progress)
    }
    index = downloadItems.value.findIndex(item => item.id === progress.id)
    if (index >= 0) {
      downloadItems.value[index] = progress
    }
  });

  dragDropEvent = await getCurrentWebviewWindow().onDragDropEvent(async ({payload}) => {
    const {type} = payload
    if (type === 'over') {
    } else if (type === 'drop') {
      await upload(payload.paths)
    } else {
    }
  })
})

watch(uploadAllCompleted, (val) => {
  if (val && !uploadNotified.value) {
    message.success('文件上传已完成')
    uploadNotified.value = true
  }
  if (!val) {
    uploadNotified.value = false
  }
})

watch(downloadAllCompleted, (val) => {
  if (val && !downloadNotified.value) {
    message.success('文件下载已完成')
    downloadNotified.value = true
  }
  if (!val) {
    downloadNotified.value = false
  }
})

onUnmounted(() => {
  transferEvent?.apply(null)
  dragDropEvent?.apply(null)
})
defineExpose({
  upload,
  download
})
</script>

<style scoped lang="less">
.progress-summary {
  position: fixed;
  bottom: 20px;
  right: 20px;
  background: rgba(255, 255, 255, 0.9);
  border: 1px solid #ddd;
  border-radius: 20px;
  padding: 8px 16px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
  backdrop-filter: blur(10px);
  display: flex;
  gap: 16px;
  cursor: pointer;
  z-index: 1000;

  .progress-item {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    color: #666;

    span {
      font-weight: bold;
      color: #333;
    }
  }
}

.progress-detail-panel {
  position: fixed;
  bottom: 70px;
  right: 20px;
  width: 400px;
  background: white;
  border: 1px solid #ddd;
  border-radius: 8px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  z-index: 1001;
  overflow: hidden;

  .progress-tabs {
    display: flex;
    border-bottom: 1px solid #eee;

    .tab {
      flex: 1;
      padding: 12px;
      text-align: center;
      cursor: pointer;
      font-size: 14px;
      color: #666;

      &.active {
        color: #1890ff;
        border-bottom: 2px solid #1890ff;
        font-weight: bold;
      }

      &:hover {
        background-color: #f5f5f5;
      }
    }
  }

  .progress-list {
    height: 400px;
    overflow-y: auto;

    .progress-item-detail {
      padding: 12px;
      border-bottom: 1px solid #eee;
      display: flex;
      flex-direction: column;
      gap: 8px;

      &:last-child {
        border-bottom: none;
      }

      .progress-info {
        .file-name {
          font-weight: bold;
          color: #333;
          margin-bottom: 4px;
        }

        .file-path {
          font-size: 12px;
          color: #999;
          word-break: break-all;
        }
      }

      .progress-status {
        display: flex;
        align-items: center;
        gap: 8px;

        .progress-bar {
          flex: 1;
          height: 6px;
          background: #f0f0f0;
          border-radius: 3px;
          overflow: hidden;

          .progress-fill {
            height: 100%;
            background: linear-gradient(90deg, #1890ff, #52c41a);
            transition: width 0.3s ease;
          }
        }

        .progress-percent {
          font-size: 12px;
          color: #666;
          min-width: 40px;
          text-align: right;
        }

        .status-badge {
          font-size: 12px;
          padding: 2px 6px;
          border-radius: 4px;

          &.uploading {
            background: #e6f7ff;
            color: #1890ff;
          }

          &.completed {
            background: #f6ffed;
            color: #52c41a;
          }

          &.failed {
            background: #fff2f0;
            color: #ff4d4f;
          }
        }
      }
    }
  }
}

.progress-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: transparent;
  z-index: 1000;
}
</style>
