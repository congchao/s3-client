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
  bucket: string;
  currentPath: string;
}

// 定义 emits
interface Emits {
  (e: 'update:visible', value: boolean): void;

  (e: 'close'): void;

  (e: 'download', file: FileItem): void;

  (e: 'parquet-to-excel', file: FileItem): void;
}

const props = defineProps<Props>();
const emit = defineEmits<Emits>();

// 内部状态
const previewLoading = ref<boolean>(false);
const previewContent = ref<string>('');
const previewType = ref<FileType>(FileType.Other);
const textPreviewTooLarge = ref<boolean>(false);
const parquetPreviewTooLarge = ref<boolean>(false);
type PreviewColumn = {
  title: string;
  dataIndex: string;
  key: string;
  ellipsis: boolean,
  resizable?: true,
  width: number,
  customCell: () => { style: Record<string, string> },
  customHeaderCell: () => { style: Record<string, string> },
}

const csvColumns = ref<PreviewColumn[]>([]);
const csvRows = ref<Record<string, unknown>[]>([]);
const csvTotalRows = ref<number>(0);
const csvError = ref<string>('');
const parquetColumns = ref<PreviewColumn[]>([]);
const parquetRows = ref<Record<string, unknown>[]>([]);
const parquetSourceRows = ref<Record<string, unknown>[]>([]);
const parquetTotalRows = ref<number>(0);
const parquetSchemaRows = ref<Record<string, unknown>[]>([]);
const parquetPage = ref<number>(1);
const parquetPageSize = ref<number>(100);
const parquetMaxPreviewSize = 10 * 1024 * 1024;
const parquetColumnWidth = 200;
const parquetColumnStyle = {
  width: `${parquetColumnWidth}px`,
  minWidth: `${parquetColumnWidth}px`,
  maxWidth: `${parquetColumnWidth}px`,
};
const parquetError = ref<string>('');
const tableColumnStyle = {
  width: '200px',
  minWidth: '200px',
  maxWidth: '200px',
};
let parquetWasmInitPromise: ReturnType<typeof initWasm> | null = null;
const previewContainer = useTemplateRef('previewContainer')
let tblHeight = ref<number>(0)
const parquetTableHeight = computed<number>(() => Math.max(220, tblHeight.value - 128));
const parquetSchemaHeight = computed<number>(() => Math.max(220, tblHeight.value - 56));

// 计算属性：控制模态框显示
const previewVisible = computed<boolean>({
  get: () => props.visible,
  set: (value) => emit('update:visible', value)
});

// 计算属性：当前预览文件
const previewFile = computed<FileItem | null>(() => props.file);

const readPreviewText = async (): Promise<string> => {
  if (!props.file) return '';
  const fileData: number[] = await fileApi.downloadFile(
      props.configId,
      props.bucket,
      `${props.currentPath}${props.file.name}`
  );
  return new TextDecoder('utf-8').decode(new Uint8Array(fileData));
};

const escapeHtml = (value: string): string => {
  return value
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#39;');
};

const renderInlineMarkdown = (value: string): string => {
  let html = escapeHtml(value);
  html = html.replace(/`([^`]+)`/g, '<code>$1</code>');
  html = html.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');
  html = html.replace(/\*([^*]+)\*/g, '<em>$1</em>');
  html = html.replace(/\[([^\]]+)]\((https?:\/\/[^)\s]+)\)/g, '<a href="$2" target="_blank" rel="noreferrer">$1</a>');
  return html;
};

const renderMarkdown = (value: string): string => {
  const lines = value.replace(/\r\n/g, '\n').split('\n');
  const html: string[] = [];
  let inCodeBlock = false;
  let listOpen = false;

  const closeList = () => {
    if (listOpen) {
      html.push('</ul>');
      listOpen = false;
    }
  };

  for (const line of lines) {
    if (line.trim().startsWith('```')) {
      closeList();
      if (inCodeBlock) {
        html.push('</code></pre>');
      } else {
        html.push('<pre><code>');
      }
      inCodeBlock = !inCodeBlock;
      continue;
    }

    if (inCodeBlock) {
      html.push(`${escapeHtml(line)}\n`);
      continue;
    }

    const heading = line.match(/^(#{1,6})\s+(.*)$/);
    if (heading) {
      closeList();
      const level = heading[1].length;
      html.push(`<h${level}>${renderInlineMarkdown(heading[2])}</h${level}>`);
      continue;
    }

    const listItem = line.match(/^\s*[-*]\s+(.*)$/);
    if (listItem) {
      if (!listOpen) {
        html.push('<ul>');
        listOpen = true;
      }
      html.push(`<li>${renderInlineMarkdown(listItem[1])}</li>`);
      continue;
    }

    closeList();
    if (line.trim()) {
      html.push(`<p>${renderInlineMarkdown(line)}</p>`);
    }
  }

  closeList();
  if (inCodeBlock) html.push('</code></pre>');
  return html.join('');
};

const parseDelimitedText = (value: string, delimiter: string): string[][] => {
  const rows: string[][] = [];
  let row: string[] = [];
  let cell = '';
  let inQuotes = false;

  for (let i = 0; i < value.length; i += 1) {
    const char = value[i];
    const next = value[i + 1];

    if (char === '"') {
      if (inQuotes && next === '"') {
        cell += '"';
        i += 1;
      } else {
        inQuotes = !inQuotes;
      }
    } else if (char === delimiter && !inQuotes) {
      row.push(cell);
      cell = '';
    } else if ((char === '\n' || char === '\r') && !inQuotes) {
      if (char === '\r' && next === '\n') i += 1;
      row.push(cell);
      rows.push(row);
      row = [];
      cell = '';
    } else {
      cell += char;
    }
  }

  if (cell || row.length > 0) {
    row.push(cell);
    rows.push(row);
  }

  return rows.filter((item) => item.some((cellValue) => cellValue !== ''));
};

const buildTableColumn = (name: string, index: number): PreviewColumn => ({
  title: name || `列 ${index + 1}`,
  dataIndex: `col_${index}`,
  key: `col_${index}`,
  ellipsis: true,
  width: 200,
  customCell: () => ({style: tableColumnStyle}),
  customHeaderCell: () => ({style: tableColumnStyle}),
});

const formatComplexCellValue = (value: unknown): unknown => {
  if (value === null || value === undefined) return '';
  if (typeof value === 'bigint') return value.toString();
  if (value instanceof Date) return value.toISOString();
  if (value instanceof Uint8Array) return `Uint8Array(${value.length})`;
  if (Array.isArray(value) || typeof value === 'object') {
    return JSON.stringify(value, (_key, nestedValue) => {
      if (typeof nestedValue === 'bigint') return nestedValue.toString();
      if (nestedValue instanceof Date) return nestedValue.toISOString();
      if (nestedValue instanceof Uint8Array) return `Uint8Array(${nestedValue.length})`;
      return nestedValue;
    });
  }
  return value;
};

const updateParquetPageRows = (): void => {
  const start = (parquetPage.value - 1) * parquetPageSize.value;
  const rows = parquetSourceRows.value.slice(start, start + parquetPageSize.value);
  parquetRows.value = rows.map((row, index) => ({
    __key: start + index,
    ...row,
  }));
};

// 预览文件内容
const previewFileContent = async (): Promise<void> => {
  if (!props.file || props.file.isDir) return;

  previewLoading.value = true;
  csvColumns.value = [];
  csvRows.value = [];
  csvTotalRows.value = 0;
  csvError.value = '';
  parquetColumns.value = [];
  parquetRows.value = [];
  parquetSourceRows.value = [];
  parquetTotalRows.value = 0;
  parquetSchemaRows.value = [];
  parquetPage.value = 1;
  parquetError.value = '';
  textPreviewTooLarge.value = false;
  parquetPreviewTooLarge.value = false;

  const fileType = getFileType(props.file.name);

  try {

    if ([FileType.Text, FileType.Csv, FileType.Json, FileType.Markdown].includes(fileType)) {
      const size = props.file.size ?? 0;
      const maxSize = 5 * 1024 * 1024;
      if (size > maxSize) {
        previewContent.value = `文件过大（${(size / 1024 / 1024).toFixed(2)} MB），为避免卡顿暂不支持预览`;
        textPreviewTooLarge.value = true;
        previewType.value = fileType;
        return;
      }

      const text = await readPreviewText();
      if (fileType === FileType.Json) {
        previewContent.value = JSON.stringify(JSON.parse(text), null, 2);
      } else if (fileType === FileType.Markdown) {
        previewContent.value = renderMarkdown(text);
      } else if (fileType === FileType.Csv) {
        const delimiter = props.file.name.toLowerCase().endsWith('.tsv') ? '\t' : ',';
        const rows = parseDelimitedText(text, delimiter);
        if (rows.length === 0) {
          csvError.value = 'CSV 内容为空';
        } else {
          const headers = rows[0].map((header, index) => header || `列 ${index + 1}`);
          csvColumns.value = headers.map(buildTableColumn);
          csvRows.value = rows.slice(1).map((row, rowIndex) => {
            const record: Record<string, unknown> = {__key: rowIndex};
            headers.forEach((_header, index) => {
              record[`col_${index}`] = row[index] ?? '';
            });
            return record;
          });
          csvTotalRows.value = csvRows.value.length;
        }
      } else {
        previewContent.value = text;
      }
    } else if (fileType === FileType.Image || fileType === FileType.Video) {
      // 对于图片和视频，获取授权访问链接
      previewContent.value = await fileApi.getFilePreviewUrl(
          props.configId,
          props.bucket,
          `${props.currentPath}${props.file.name.replace('/', '')}`
      );
    } else if (fileType === FileType.Parquet) {
      const size = props.file.size ?? 0;
      if (size > parquetMaxPreviewSize) {
        parquetPreviewTooLarge.value = true;
        previewType.value = fileType;
        return;
      }

      const fileData: number[] = await fileApi.downloadFile(
          props.configId,
          props.bucket,
          `${props.currentPath}${props.file.name}`
      );
      const parquetBuffer = new Uint8Array(fileData);

      if (!parquetWasmInitPromise) {
        parquetWasmInitPromise = initWasm(parquetWasmUrl);
      }
      await parquetWasmInitPromise;

      const wasmTable = readParquet(parquetBuffer);
      const arrowTable = tableFromIPC(wasmTable.intoIPCStream());

      const fields = arrowTable.schema.fields;
      const fieldNames = fields.map((f: { name: string }) => f.name);
      parquetSchemaRows.value = fields.map((field: { name: string; type: unknown; nullable?: boolean }, index: number) => ({
        __key: index,
        name: field.name,
        type: String(field.type),
        nullable: field.nullable ? '是' : '否',
      }));
      parquetColumns.value = fieldNames.map((name: string) => ({
        title: name,
        dataIndex: name,
        key: name,
        ellipsis: true,
        resizable: true,
        width: parquetColumnWidth,
        customCell: () => ({style: parquetColumnStyle}),
        customHeaderCell: () => ({style: parquetColumnStyle}),
      }));

      const allRows = arrowTable.toArray();
      parquetTotalRows.value = allRows.length;
      parquetSourceRows.value = allRows.map((row: Record<string, unknown>) => {
        const record: Record<string, unknown> = {};
        for (const name of fieldNames) {
          record[name] = formatComplexCellValue((row as Record<string, unknown>)[name]);
        }
        return record;
      });
      updateParquetPageRows();
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
    await nextTick(() => {
      tblHeight.value = (previewContainer.value?.clientHeight || 450) - 65
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
  parquetPreviewTooLarge.value = false;
  csvColumns.value = [];
  csvRows.value = [];
  csvTotalRows.value = 0;
  csvError.value = '';
  parquetColumns.value = [];
  parquetRows.value = [];
  parquetSourceRows.value = [];
  parquetTotalRows.value = 0;
  parquetSchemaRows.value = [];
  parquetPage.value = 1;
  parquetError.value = '';
  emit('close');
};

// 下载预览文件
const downloadPreviewFile = async (): Promise<void> => {
  if (props.file) {
    emit('download', props.file);
  }
};

const exportParquetToExcel = async (): Promise<void> => {
  if (props.file && getFileType(props.file.name) === FileType.Parquet) {
    emit('parquet-to-excel', props.file);
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

      <!-- JSON 预览 -->
      <div v-if="previewType === FileType.Json" class="preview-text-container">
        <pre class="preview-text">{{ previewContent }}</pre>
      </div>

      <!-- Markdown 预览 -->
      <div v-if="previewType === FileType.Markdown" class="preview-markdown-container">
        <article class="preview-markdown" v-html="previewContent"></article>
      </div>

      <!-- CSV 预览 -->
      <div v-if="previewType === FileType.Csv" class="preview-csv-container">
        <div v-if="csvError" class="preview-parquet-error">
          <p>{{ csvError }}</p>
        </div>
        <div v-else class="preview-csv-content">
          <div class="preview-parquet-meta">
            <span>共 {{ csvTotalRows }} 行</span>
          </div>
          <a-table
              class="fixed-preview-table"
              :columns="csvColumns"
              :data-source="csvRows"
              :pagination="{ pageSize: 100, showSizeChanger: true }"
              bordered
              size="small"
              row-key="__key"
              table-layout="fixed"
              :scroll="{ x: 'max-content', y: tblHeight }"
          />
        </div>
      </div>

      <!-- Parquet 预览 -->
      <div v-if="previewType === FileType.Parquet" class="preview-parquet-container">
        <div v-if="parquetPreviewTooLarge" class="preview-parquet-large">
          <FileOutlined :style="{ fontSize: '48px', color: '#1890ff' }"/>
          <p>文件超过 10MB，暂不支持在线预览</p>
          <a-button type="primary" @click="downloadPreviewFile">
            <DownloadOutlined/>
            点击下载
          </a-button>
        </div>
        <div v-else-if="parquetError" class="preview-parquet-error">
          <p>Parquet 解析失败：{{ parquetError }}</p>
        </div>
        <div class="preview-parquet-content" v-else>
          <a-tabs class="parquet-tabs" size="small">
            <a-tab-pane key="data" tab="数据">
              <div class="parquet-data-pane">
                <div class="preview-parquet-meta">
                  <span>共 {{ parquetTotalRows }} 行</span>
                </div>
                <div class="parquet-table-wrap">
                  <a-table
                      class="fixed-preview-table parquet-preview-table"
                      :columns="parquetColumns"
                      :data-source="parquetRows"
                      :pagination="false"
                      bordered
                      size="small"
                      row-key="__key"
                      table-layout="fixed"
                      :scroll="{ x:  'max-content', y: parquetTableHeight }"
                  />
                </div>
                <a-flex class="parquet-pagination" justify="end">
                  <a-pagination
                      v-model:current="parquetPage"
                      v-model:page-size="parquetPageSize"
                      :total="parquetTotalRows"
                      :page-size-options="['50', '100', '200', '500']"
                      show-size-changer
                      show-less-items
                      @change="updateParquetPageRows"
                      @showSizeChange="updateParquetPageRows"
                  />
                </a-flex>
              </div>
            </a-tab-pane>
            <a-tab-pane key="schema" tab="Schema">
              <a-table
                  :columns="[
                    { title: '字段名', dataIndex: 'name', key: 'name' },
                    { title: '类型', dataIndex: 'type', key: 'type' },
                    { title: '允许为空', dataIndex: 'nullable', key: 'nullable', width: 120 }
                  ]"
                  :data-source="parquetSchemaRows"
                  :pagination="false"
                  bordered
                  size="small"
                  row-key="__key"
                  :scroll="{ y: parquetSchemaHeight }"
              />
            </a-tab-pane>
          </a-tabs>
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
      <a-button
          v-if="previewType === FileType.Parquet && !parquetPreviewTooLarge"
          @click="exportParquetToExcel"
      >
        Parquet 转 Excel
      </a-button>
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

.preview-markdown-container {
  width: 100%;
  height: 100%;
  overflow: auto;
  padding: 24px;
  background: #fff;
}

.preview-markdown {
  max-width: 920px;
  margin: 0 auto;
  color: #1f2328;
  line-height: 1.7;

  :deep(h1), :deep(h2), :deep(h3), :deep(h4), :deep(h5), :deep(h6) {
    margin: 18px 0 10px;
    font-weight: 600;
  }

  :deep(p) {
    margin: 0 0 12px;
  }

  :deep(pre) {
    background: #f6f8fa;
    border: 1px solid #d0d7de;
    border-radius: 6px;
    padding: 12px;
    overflow: auto;
  }

  :deep(code) {
    background: #f6f8fa;
    border-radius: 4px;
    padding: 2px 4px;
  }
}

.preview-csv-container, .preview-csv-content {
  width: 100%;
  height: 100%;
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
  min-height: 0;
}

.preview-parquet-large {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 16px;
  color: #666;
}

.preview-parquet-content, .preview-csv-content {
  overflow: hidden;

  :deep(.parquet-tabs),
  :deep(.parquet-tabs > .ant-tabs-content-holder),
  :deep(.parquet-tabs > .ant-tabs-content-holder > .ant-tabs-content),
  :deep(.parquet-tabs > .ant-tabs-content-holder > .ant-tabs-content > .ant-tabs-tabpane) {
    height: 100%;
    min-height: 0;
  }

  :deep(.fixed-preview-table .ant-table table) {
    table-layout: fixed !important;
  }

  :deep(.fixed-preview-table .ant-table) {
    border-color: #d9d9d9;
  }

  :deep(.fixed-preview-table .ant-table-thead > tr > th) {
    border-color: #d9d9d9;
    background: #fafafa;
    font-weight: 600;
  }

  :deep(.fixed-preview-table .ant-table-tbody > tr > td) {
    border-color: #e5e5e5;
  }

  :deep(.fixed-preview-table .ant-table-thead > tr > th),
  :deep(.fixed-preview-table .ant-table-tbody > tr > td) {
    width: 200px !important;
    min-width: 200px !important;
    max-width: 200px !important;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
}

.parquet-data-pane {
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.parquet-table-wrap {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.parquet-pagination {
  flex: 0 0 auto;
  padding: 12px 0 0;
  background: #fff;
}

.preview-container {
  flex: 1;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
}

.preview-download-row {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
