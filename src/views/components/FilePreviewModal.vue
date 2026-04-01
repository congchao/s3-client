<script setup lang="ts">
import {computed, ref, watch, useTemplateRef, nextTick} from 'vue';
import {FileItem} from '@/types';
import {FileType} from '@/types/constants';
import initWasm, {readParquet} from 'parquet-wasm';
import parquetWasmUrl from 'parquet-wasm/esm/parquet_wasm_bg.wasm?url';
import {tableFromIPC} from 'apache-arrow';
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
const textPreviewTooLarge = ref<boolean>(false);
const parquetColumns = ref<{
  title: string;
  dataIndex: string;
  key: string;
  ellipsis: boolean,
  resizable: true,
  width: number
}[]>([]);
const parquetRows = ref<Record<string, unknown>[]>([]);
const parquetTotalRows = ref<number>(0);
const parquetRowLimit = 200;
const parquetError = ref<string>('');
let parquetWasmInitPromise: Promise<void> | null = null;
const previewContainer = useTemplateRef('previewContainer')
let tblHeight = ref<number>(0)

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
  parquetColumns.value = [];
  parquetRows.value = [];
  parquetTotalRows.value = 0;
  parquetError.value = '';
  textPreviewTooLarge.value = false;

  const fileType = getFileType(props.file.name);

  try {

    if (fileType === FileType.Text) {
      const size = props.file.size ?? 0;
      const maxSize = 5 * 1024 * 1024;
      if (size > maxSize) {
        previewContent.value = `文件过大（${(size / 1024 / 1024).toFixed(2)} MB），为避免卡顿暂不支持预览`;
        textPreviewTooLarge.value = true;
        previewType.value = fileType;
        return;
      }
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
          `${props.currentPath}${props.file.name.replace('/', '')}`
      );
    } else if (fileType === FileType.Parquet) {
      const fileData: number[] = await fileApi.downloadFile(
          props.configId,
          `${props.currentPath}${props.file.name}`
      );
      const parquetBuffer = new Uint8Array(fileData);

      if (!parquetWasmInitPromise) {
        parquetWasmInitPromise = initWasm(parquetWasmUrl);
      }
      await parquetWasmInitPromise;

      const wasmTable = readParquet(parquetBuffer);
      const arrowTable = tableFromIPC(wasmTable.intoIPCStream());

      const fieldNames = arrowTable.schema.fields.map((f) => f.name);
      parquetColumns.value = fieldNames.map((name) => ({
        title: name,
        dataIndex: name,
        key: name,
        ellipsis: true,
        width: 150,
      }));

      const formatCellValue = (value: unknown): unknown => {
        if (value === null || value === undefined) return '';
        if (typeof value === 'bigint') return value.toString();
        if (value instanceof Date) return value.toISOString();
        if (value instanceof Uint8Array) return `Uint8Array(${value.length})`;
        if (Array.isArray(value)) return JSON.stringify(value);
        if (typeof value === 'object') return JSON.stringify(value);
        return value;
      };

      const allRows = arrowTable.toArray();
      parquetTotalRows.value = allRows.length;
      parquetRows.value = allRows.slice(0, parquetRowLimit).map((row, index) => {
        const record: Record<string, unknown> = {__key: index};
        for (const name of fieldNames) {
          record[name] = formatCellValue((row as Record<string, unknown>)[name]);
        }
        return record;
      });
    }

    previewType.value = fileType;
  } catch (error) {
    console.error('预览文件失败:', error);
    if (fileType === FileType.Parquet) {
      parquetError.value = error instanceof Error ? error.message : 'Parquet 解析失败';
      previewType.value = FileType.Parquet;
    } else {
      message.error('预览文件失败！');
      emit('update:visible', false);
    }
  } finally {
    previewLoading.value = false;
    nextTick(() => {
      tblHeight.value = (previewContainer.value?.clientHeight || 450) - 65
      console.log(tblHeight.value)
    })
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
  textPreviewTooLarge.value = false;
  parquetColumns.value = [];
  parquetRows.value = [];
  parquetTotalRows.value = 0;
  parquetError.value = '';
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
      width="100%"
      wrap-class-name="full-modal"
      @cancel="closePreview"
  >
    <div v-if="previewLoading" class="preview-loading">
      <a-spin size="large"/>
    </div>

    <div class="preview-container" ref="previewContainer" v-else>
      <!-- 图片预览 -->
      <img
          v-if="previewType === FileType.Image"
          :src="previewContent"
          :alt="previewFile?.name"
          class="preview-image"
          @error="handleImageError"
      />

      <!-- 视频预览 -->
      <video
          v-if="previewType === FileType.Video"
          :src="previewContent"
          controls
          class="preview-video"
          @error="handleVideoError"
      >
        您的浏览器不支持视频播放
      </video>

      <!-- 文本预览 -->
      <div v-if="previewType === FileType.Text" class="preview-text-container">
        <pre class="preview-text" :class="{ 'preview-text-muted': textPreviewTooLarge }">{{ previewContent }}</pre>
      </div>

      <!-- Parquet 预览 -->
      <div v-if="previewType === FileType.Parquet" class="preview-parquet-container">
        <div v-if="parquetError" class="preview-parquet-error">
          <p>Parquet 解析失败：{{ parquetError }}</p>
        </div>
        <div class="preview-parquet-content" v-else>
          <div class="preview-parquet-meta">
            <span>共 {{ parquetTotalRows }} 行</span>
            <span v-if="parquetTotalRows > parquetRowLimit">仅预览前 {{ parquetRowLimit }} 行</span>
          </div>
          <a-table
              :columns="parquetColumns"
              :data-source="parquetRows"
              :pagination="false"
              size="small"
              row-key="__key"
              :scroll="{ x:  'max-content',y: tblHeight }"
          />
        </div>
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
    <div class="preview-download-row" v-if="previewType !== FileType.Other">
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
  height: 100%;
  width: 100%;
}

.preview-image {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
}

.preview-video {
  max-width: 100%;
  max-height: 100%;
  width: auto;
  height: auto;
}

.preview-text-container {
  padding: 16px;
  max-height: 100%;
  overflow: auto;

  .preview-text {
    margin: 0;
    font-family: 'Courier New', monospace;
    font-size: 14px;
    line-height: 1.5;
    white-space: pre-wrap;
    word-wrap: break-word;
  }

  .preview-text-muted {
    color: #999;
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

.preview-parquet-container, .preview-parquet-content {
  width: 100%;
  height: 100%;
}

.preview-container {
  flex: 1;
  overflow: hidden;
  overflow-y: auto;
  display: flex;
  align-items: center;
  justify-content: center;
}

.preview-download-row {
  display: flex;
  justify-content: flex-end;
}
</style>
