<template>
  <div
      v-if="visible"
      class="custom-context-menu context-menu-style"
      :style="{ top: y + 'px', left: x + 'px' }"
  >
    <a-button
        type="link"
        size="small"
        @click="handleDownload"
    >
      下载
    </a-button>
    <a-button
        type="link"
        size="small"
        @click="handleDelete"
    >
      删除
    </a-button>
    <a-button
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
import {ref} from 'vue';

// 定义 props
interface Props {
  visible: boolean;
  x: number;
  y: number;
  file: FileItem | null;
}

// 定义 emits
interface Emits {
  (e: 'update:visible', value: boolean): void;

  (e: 'download', file: FileItem): void;

  (e: 'delete', file: FileItem): void;

  (e: 'copy', file: FileItem): void;
}

const props = defineProps<Props>();
const emit = defineEmits<Emits>();

// 内部状态
const contextVisible = ref<boolean>(props.visible);


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

// 隐藏菜单
const hideMenu = () => {
  contextVisible.value = false;
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
