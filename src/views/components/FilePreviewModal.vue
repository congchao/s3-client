

<script setup lang="ts">
import {computed, ref, watch} from 'vue';
import {FileItem} from '@/types';
import {FileType} from '@/types/constants';
import {getFileType} from '@/utils/utils';
import {message} from 'ant-design-vue';
import {DownloadOutlined, FileOutlined} from '@ant-design/icons-vue';
import {fileApi} from '@/services/file';

// 定义 props
interface Props {
  visible: boolean;
  file: FileItem | null;
  configId: string;
  currentPath: string;
}

// 定义 emits
interface Emits {
  (e: 'update:visible', value: boolean): void;
  (e: 'close'): void;
  (e: 'download', file: FileItem): void;
}

const props = defineProps<Props>();
const emit = defineEmits<Emits>();

// 内部状态
const previewLoading = ref<boolean>(false);
const previewContent = ref<string>('');
const previewType = ref<FileType>(FileType.Other);

// 计算属性：控制模态框显示
const previewVisible = computed<boolean>({
  get: () => props.visible,
  set: (value) => emit('update:visible', value)
});

// 计算属性：当前预览文件
const previewFile = computed<FileItem | null>(() => props.file);

// 预览文件内容
const previewFileContent = async (): Promise<void> => {
  if (!props.file || props.file.isDir) return;

  previewLoading.value = true;

  try {
    const fileType = getFileType(props.file.name);

    if (fileType === FileType.Text) {
      // 对于文本文件，需要下载内容进行预览
      const fileData: number[] = await fileApi.downloadFile(
        props.configId,
        `${props.currentPath}${props.file.name}`
      );

      // 转换为文本内容
      const uint8Array = new Uint8Array(fileData);
      const decoder = new TextDecoder('utf-8');
      previewContent.value = decoder.decode(uint8Array);
    } else if (fileType === FileType.Image || fileType === FileType.Video) {
      // 对于图片和视频，获取授权访问链接
      previewContent.value = await fileApi.getFilePreviewUrl(
        props.configId,
        `${props.currentPath}${props.file.name}`
      );
    }

    previewType.value = fileType;
  } catch (error) {
    console.error('预览文件失败:', error);
    message.error('预览文件失败！');
    emit('update:visible', false);
  } finally {
    previewLoading.value = false;
  }
};

// 关闭预览
const closePreview = (): void => {
  // 释放 Blob URL（如果之前使用的是 Blob URL）
  if (previewContent.value && !previewContent.value.startsWith('http')) {
    URL.revokeObjectURL(previewContent.value);
  }

  previewContent.value = '';
  previewType.value = FileType.Other;
  emit('close');
};

// 下载预览文件
const downloadPreviewFile = async (): Promise<void> => {
  if (props.file) {
    emit('download', props.file);
  }
};

// 处理媒体文件错误
const handleImageError = () => {
  console.error('图片加载失败');
  message.error('图片加载失败');
};

const handleVideoError = () => {
  console.error('视频加载失败');
  message.error('视频加载失败');
};

// 监听 props 变化
watch(
  () => props.visible,
  (newValue) => {
    if (newValue && props.file) {
      previewFileContent();
    }
  }
);
</script>

<template>
  <a-modal
      v-model:open="previewVisible"
      :title="previewFile?.name"
      :footer="null"
      :width="previewType === FileType.Text ? '800px' : '800px'"
      :body-style="previewType === FileType.Text ? { padding: '0', height: '70vh' } : {}"
      @cancel="closePreview"
  >
    <div v-if="previewLoading" class="preview-loading">
      <a-spin size="large"/>
    </div>

    <div v-else>
      <!-- 图片预览 -->
      <div v-if="previewType === FileType.Image" class="preview-image-container">
        <img
            :src="previewContent"
            :alt="previewFile?.name"
            class="preview-image"
            @error="handleImageError"
        />
      </div>

      <!-- 视频预览 -->
      <div v-if="previewType === FileType.Video" class="preview-video-container">
        <video
            :src="previewContent"
            controls
            class="preview-video"
            @error="handleVideoError"
        >
          您的浏览器不支持视频播放
        </video>
      </div>

      <!-- 文本预览 -->
      <div v-if="previewType === FileType.Text" class="preview-text-container">
        <pre class="preview-text">{{ previewContent }}</pre>
      </div>

      <!-- 其他类型文件提示 -->
      <div v-if="previewType === FileType.Other" class="preview-other-container">
        <div class="preview-other-content">
          <FileOutlined :style="{ fontSize: '48px', color: '#1890ff' }"/>
          <p class="preview-other-text">无法预览此文件类型</p>
          <a-button type="primary" @click="downloadPreviewFile">
            <DownloadOutlined/>
            点击下载
          </a-button>
        </div>
      </div>
    </div>

    <!-- 下载按钮 -->
    <div class="preview-download-btn" v-if="previewType !== FileType.Other">
      <a-button type="primary" @click="downloadPreviewFile">
        <DownloadOutlined/>
        下载文件
      </a-button>
    </div>
  </a-modal>
</template>

<style scoped lang="less">
.preview-loading {
  display: flex;
  justify-content: center;
  align-items: center;
  height: 300px;
}

.preview-image-container {
  display: flex;
  justify-content: center;
  align-items: center;
  padding: 16px;
  height: 60vh;

  .preview-image {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
  }
}

.preview-video-container {
  display: flex;
  justify-content: center;
  align-items: center;
  padding: 16px;
  height: 60vh;

  .preview-video {
    max-width: 100%;
    max-height: 100%;
    width: auto;
    height: auto;
  }
}

.preview-text-container {
  padding: 16px;
  max-height: 70vh;
  overflow: auto;

  .preview-text {
    margin: 0;
    font-family: 'Courier New', monospace;
    font-size: 14px;
    line-height: 1.5;
    white-space: pre-wrap;
    word-wrap: break-word;
  }
}

.preview-other-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 40px 20px;

  .preview-other-content {
    text-align: center;

    .preview-other-text {
      margin: 16px 0;
      font-size: 16px;
      color: #666;
    }
  }
}

.preview-download-btn {
  position: absolute;
  bottom: 16px;
  right: 16px;
}
</style>
