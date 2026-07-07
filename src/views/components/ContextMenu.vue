<template>
  <div
      v-if="visible"
      ref="menuRef"
      class="custom-context-menu context-menu-style"
      :style="{ top: menuPosition.y + 'px', left: menuPosition.x + 'px' }"
  >
    <a-button
        v-if="menuSettings.download"
        type="link"
        size="small"
        @click="handleDownload"
    >
      下载
    </a-button>
    <a-button
        v-if="menuSettings.rename"
        type="link"
        size="small"
        @click="handleRename"
    >
      重命名
    </a-button>
    <a-button
        v-if="menuSettings.moveItem"
        type="link"
        size="small"
        @click="handleMove"
    >
      移动
    </a-button>
    <a-button
        v-if="menuSettings.duplicate"
        type="link"
        size="small"
        @click="handleDuplicate"
    >
      复制对象
    </a-button>
    <a-button
        v-if="!file?.isDir && menuSettings.share"
        type="link"
        size="small"
        @click="handleShare"
    >
      预签名链接
    </a-button>
    <a-button
        v-if="menuSettings.parquetToExcel && (file?.isDir || isParquetFile)"
        type="link"
        size="small"
        @click="handleParquetToExcel"
    >
      Parquet 转 Excel
    </a-button>
    <a-button
        v-if="menuSettings.delete"
        type="link"
        size="small"
        @click="handleDelete"
    >
      删除
    </a-button>
    <a-button
        v-if="menuSettings.copyPath"
        type="link"
        size="small"
        @click="handleCopy"
    >
      复制路径
    </a-button>
  </div>
</template>

<script setup lang="ts">
import {FileItem} from '@/types';
import {computed, nextTick, reactive, ref, watch} from 'vue';
import {ContextMenuSettings} from '@/types';

// 定义 props
interface Props {
  visible: boolean;
  x: number;
  y: number;
  file: FileItem | null;
  settings: ContextMenuSettings;
}

// 定义 emits
interface Emits {
  (e: 'update:visible', value: boolean): void;

  (e: 'download', file: FileItem): void;

  (e: 'delete', file: FileItem): void;

  (e: 'copy', file: FileItem): void;

  (e: 'rename', file: FileItem): void;

  (e: 'move', file: FileItem): void;

  (e: 'duplicate', file: FileItem): void;

  (e: 'share', file: FileItem): void;

  (e: 'parquet-to-excel', file: FileItem): void;
}

const props = defineProps<Props>();
const emit = defineEmits<Emits>();

// 内部状态
const menuRef = ref<HTMLElement | null>(null);
const menuPosition = reactive({x: props.x, y: props.y});
const menuSettings = computed(() => props.settings);
const isParquetFile = computed(() => props.file?.name.toLowerCase().endsWith('.parquet') ?? false);

const updateMenuPosition = async (): Promise<void> => {
  menuPosition.x = props.x;
  menuPosition.y = props.y;
  if (!props.visible) return;

  await nextTick();

  const menuEl = menuRef.value;
  if (!menuEl) return;

  const margin = 8;
  const width = menuEl.offsetWidth;
  const height = menuEl.offsetHeight;
  const maxX = window.innerWidth - width - margin;
  const maxY = window.innerHeight - height - margin;

  menuPosition.x = Math.max(margin, Math.min(props.x, maxX));
  menuPosition.y = Math.max(margin, Math.min(props.y, maxY));
};

watch(
    () => [props.visible, props.x, props.y, props.file, props.settings] as const,
    () => {
      updateMenuPosition();
    },
    {immediate: true}
);

// 处理下载事件
const handleDownload = () => {
  if (props.file) {
    emit('download', props.file);
    hideMenu();
  }
};

// 处理删除事件
const handleDelete = () => {
  if (props.file) {
    emit('delete', props.file);
    hideMenu();
  }
};

// 处理复制路径事件
const handleCopy = () => {
  if (props.file) {
    emit('copy', props.file);
    hideMenu();
  }
};

const handleRename = () => {
  if (props.file) {
    emit('rename', props.file);
    hideMenu();
  }
};

const handleMove = () => {
  if (props.file) {
    emit('move', props.file);
    hideMenu();
  }
};

const handleDuplicate = () => {
  if (props.file) {
    emit('duplicate', props.file);
    hideMenu();
  }
};

const handleShare = () => {
  if (props.file) {
    emit('share', props.file);
    hideMenu();
  }
};

const handleParquetToExcel = () => {
  if (props.file) {
    emit('parquet-to-excel', props.file);
    hideMenu();
  }
};

// 隐藏菜单
const hideMenu = () => {
  emit('update:visible', false);
};
</script>

<style scoped lang="less">
.custom-context-menu {
  position: fixed;
  z-index: 1000;
  background: white;
  border: 1px solid #d9d9d9;
  border-radius: 4px;
  box-shadow: 0 6px 16px 0 rgba(0, 0, 0, 0.08);
  min-width: 100px;
  padding: 4px 0;
}

.context-menu-style {
  .ant-btn-link {
    width: 100%;
    text-align: left;
    padding: 6px 12px;
    border-radius: 0;
    margin: 0;

    &:hover {
      background-color: #f5f5f5;
    }
  }
}
</style>
