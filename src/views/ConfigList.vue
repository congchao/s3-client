<script setup lang="ts">
import {h, nextTick, onBeforeUnmount, onMounted, ref} from 'vue'
import {useRouter} from 'vue-router'
import {message, Modal} from 'ant-design-vue'
import {PlusOutlined} from '@ant-design/icons-vue'
import Sortable from 'sortablejs'
import {OssConfig} from '@/types/index.d'
import {configApi} from '@/services/config.ts'

const router = useRouter()
const loading = ref<boolean>(false)
const configList = ref<OssConfig[]>([])
const configListBodyRef = ref<HTMLElement | null>(null)
let sortableInstance: Sortable | null = null

const loadConfigList = async (): Promise<void> => {
  loading.value = true
  try {
    const configs: OssConfig[] = await configApi.getConfig()
    configList.value = configs.map((config,) => ({
      id: config.id,
      name: config.name,
      provider: config.provider,
      region: config.region,
      endpoint: config.endpoint,
      bucket: config.bucket,
      pathStyle: config.pathStyle,
      sort: config.sort || 0,
    } as OssConfig))
  } catch (error) {
    console.error('加载配置列表失败:', error)
    message.error('加载配置列表失败！')
  } finally {
    loading.value = false
  }
}

const persistSortOrder = async (): Promise<void> => {
  try {
    await configApi.saveConfigSort(configList.value.map((config) => config.id))
    configList.value = configList.value.map((config, index) => ({
      ...config,
      sort: (index + 1) * 1000,
    }))
    message.success('排序已保存')
  } catch (error) {
    console.error('保存排序失败:', error)
    message.error('保存排序失败')
    await loadConfigList()
  }
}

const moveConfigByIndex = async (oldIndex?: number, newIndex?: number): Promise<void> => {
  if (oldIndex === undefined || newIndex === undefined || oldIndex === newIndex) return
  const nextList = [...configList.value]
  const [draggedConfig] = nextList.splice(oldIndex, 1)
  if (!draggedConfig) return
  nextList.splice(newIndex, 0, draggedConfig)
  configList.value = nextList
  await persistSortOrder()
}

const initSortable = async (): Promise<void> => {
  await nextTick()
  if (!configListBodyRef.value || sortableInstance) return

  sortableInstance = Sortable.create(configListBodyRef.value, {
    animation: 160,
    easing: 'cubic-bezier(0.2, 0, 0, 1)',
    draggable: '.config-list-row',
    direction: 'vertical',
    ghostClass: 'config-list-row--ghost',
    chosenClass: 'config-list-row--chosen',
    dragClass: 'config-list-row--drag',
    fallbackClass: 'config-list-row--fallback',
    forceFallback: true,
    fallbackOnBody: true,
    fallbackTolerance: 2,
    fallbackOffset: {x: 0, y: 0},
    swapThreshold: 0.5,
    invertedSwapThreshold: 0.35,
    emptyInsertThreshold: 24,
    filter: '.config-operation-cell, .ant-btn, button, a',
    preventOnFilter: false,
    onClone: (event) => {
      const rect = event.item.getBoundingClientRect()
      event.clone.style.width = `${rect.width}px`
      event.clone.style.height = `${rect.height}px`
    },
    onStart: () => {
      document.body.classList.add('config-list-sorting')
    },
    onEnd: (event) => {
      document.body.classList.remove('config-list-sorting')
      void moveConfigByIndex(event.oldIndex, event.newIndex)
    },
  })
}

const destroySortable = (): void => {
  sortableInstance?.destroy()
  sortableInstance = null
  document.body.classList.remove('config-list-sorting')
}

const addConfig = (): void => {
  router.push({name: 'NewConfig'})
}

const editConfig = (record: OssConfig): void => {
  router.push({name: 'EditConfig', params: {id: record.id}})
}

const copyConfig = (record: OssConfig): void => {
  router.push({name: 'NewConfig', query: {copyId: record.id}})
}

const del = async (record: OssConfig): Promise<void> => {
  const configToDelete = configList.value.find(config => config.id === record.id)
  if (!configToDelete) {
    message.error('未找到要删除的配置')
    return
  }

  Modal.confirm({
    title: '提示',
    content: `确定删除配置 "${configToDelete.name}" 吗？`,
    okText: '确定',
    cancelText: '取消',
    onOk: async () => {
      try {
        await configApi.deleteConfig(configToDelete.id)
        message.success('删除成功！')
        await loadConfigList()
      } catch (error) {
        console.error('删除配置失败:', error)
        message.error('删除配置失败！')
      }
    },
  })
}

const testSingleConfig = async (record: OssConfig): Promise<void> => {
  try {
    message.loading('测试中...', 0)

    const configs: OssConfig[] = await configApi.getConfig()
    const config = configs.find(c => c.id === record.id)

    if (!config) {
      message.destroy()
      message.error('未找到配置信息')
      return
    }

    const result: boolean = await configApi.testConfig(config)

    message.destroy()

    if (result) {
      message.success(`【${record.name}】连接测试成功！`)
    } else {
      message.error(`【${record.name}】连接测试失败`)
    }
  } catch (error) {
    console.error('测试单个配置失败:', error)
    message.destroy()
    message.error(`测试失败: ${error instanceof Error ? error.message : '未知错误'}`)
  }
}

const enterFileManager = (record: OssConfig): void => {
  const bucket = record.bucket?.trim()
  router.push({
    name: bucket ? 'FileManager' : 'BucketList',
    params: bucket ? {id: record.id, bucket} : {id: record.id}
  })
}

onMounted(async () => {
  await loadConfigList()
  await initSortable()
})

onBeforeUnmount(() => {
  destroySortable()
})
</script>
<template>
  <div class="config-list-page page-layout">
    <a-layout-header class="header header-layout with-padding">
      <div class="header-left">
        <span class="title">OSS管理工具 - 配置列表</span>
      </div>
      <div class="header-right">
        <a-button type="primary" @click="addConfig" :icon="h(PlusOutlined)">新增配置</a-button>
      </div>
    </a-layout-header>

    <a-layout-content class="content content-layout">
      <div class="config-list-card table-card-style">
        <a-spin :spinning="loading">
          <div class="config-list-table">
            <div class="config-list-header">
              <div class="config-cell config-name-cell">配置名称</div>
              <div class="config-cell config-provider-cell">提供商</div>
              <div class="config-cell config-endpoint-cell">端点</div>
              <div class="config-cell config-bucket-cell">默认桶</div>
              <div class="config-cell config-operation-cell">操作</div>
            </div>

            <div ref="configListBodyRef" class="config-list-body">
              <div
                  v-for="record in configList"
                  :key="record.id"
                  class="config-list-row"
                  @dblclick.stop="enterFileManager(record)"
              >
                <div class="config-cell config-name-cell" :title="record.name">{{ record.name }}</div>
                <div class="config-cell config-provider-cell">
                  <a-tag>{{ record.provider }}</a-tag>
                </div>
                <div class="config-cell config-endpoint-cell" :title="record.endpoint">{{ record.endpoint }}</div>
                <div class="config-cell config-bucket-cell" :title="record.bucket || '未设置'">
                  {{ record.bucket || '未设置' }}
                </div>
                <div class="config-cell config-operation-cell">
                  <a-space>
                    <a-button type="link" size="small" @click.stop="editConfig(record)">编辑</a-button>
                    <a-button type="link" size="small" @click.stop="testSingleConfig(record)">测试</a-button>
                    <a-button type="link" size="small" @click.stop="copyConfig(record)">复制</a-button>
                    <a-button type="link" danger size="small" @click.stop="del(record)">删除</a-button>
                  </a-space>
                </div>
              </div>
              <a-empty v-if="!configList.length" class="config-list-empty" description="暂无配置"/>
            </div>
          </div>
        </a-spin>
      </div>
    </a-layout-content>
  </div>
</template>
<style lang="less">
.config-list-card {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.config-list-card .ant-spin-nested-loading,
.config-list-card .ant-spin-container {
  height: 100%;
}

.config-list-table {
  display: flex;
  flex-direction: column;
  height: 100%;
  border: 1px solid #f0f0f0;
  border-radius: 6px;
  overflow: hidden;
  background: #fff;
  user-select: none;
  -webkit-user-select: none;
}

.config-list-header,
.config-list-row {
  display: flex;
  align-items: center;
  width: 100%;
}

.config-list-header {
  flex: 0 0 44px;
  background: #fafafa;
  border-bottom: 1px solid #f0f0f0;
  color: #1f2937;
  font-weight: 600;
}

.config-list-body {
  flex: 1;
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
}

.config-list-row {
  min-height: 54px;
  width: 100%;
  border-bottom: 1px solid #f0f0f0;
  box-sizing: border-box;
  cursor: grab;
  user-select: none;
  -webkit-user-select: none;
  touch-action: none;
  transition: background-color 0.14s ease, opacity 0.14s ease, box-shadow 0.14s ease;
}

.config-list-row:hover {
  background: #f8fbff;
}

.config-list-row:active {
  cursor: grabbing;
}

.config-list-row--ghost {
  background: #f6f8fb;
  opacity: 0.35;
}

.config-list-row--chosen {
  background: #f8fbff;
  cursor: grabbing;
}

.config-list-row--drag,
.config-list-row--fallback {
  box-sizing: border-box;
  display: flex;
  align-items: center;
  border: 1px solid #e6edf7;
  border-radius: 4px;
  background: #fff;
  box-shadow: 0 10px 24px rgba(15, 23, 42, 0.16);
  opacity: 0.96;
  cursor: grabbing;
  pointer-events: none;
  z-index: 9999;
}

body.config-list-sorting {
  cursor: grabbing;
  user-select: none;
  -webkit-user-select: none;
}

.config-cell {
  min-width: 0;
  padding: 10px 16px;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.config-name-cell {
  flex: 1 1 220px;
}

.config-provider-cell {
  flex: 0 0 100px;
}

.config-endpoint-cell {
  flex: 0 1 300px;
}

.config-bucket-cell {
  flex: 0 0 150px;
}

.config-operation-cell {
  flex: 0 0 235px;
}

.config-operation-cell .ant-space {
  flex-wrap: nowrap;
}

.config-list-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 240px;
}
</style>
