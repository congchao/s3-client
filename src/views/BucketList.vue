<script setup lang="ts">
import {h, onMounted, ref} from 'vue'
import {useRoute, useRouter} from 'vue-router'
import {message} from 'ant-design-vue'
import {ArrowLeftOutlined, ReloadOutlined} from '@ant-design/icons-vue'
import dayjs from 'dayjs'
import {BucketInfo, OssConfig} from '@/types'
import {configApi} from '@/services/config.ts'
import {fileApi} from '@/services/file.ts'

const route = useRoute()
const router = useRouter()
const configId = route.params.id as string
const loading = ref<boolean>(false)
let loadVersion = 0
const config = ref<OssConfig | null>(null)
const bucketList = ref<BucketInfo[]>([])

const tableColumns = [
  {
    title: '桶名称',
    dataIndex: 'name',
    key: 'name',
    ellipsis: true,
  },
  {
    title: '创建时间',
    dataIndex: 'creationDate',
    key: 'creationDate',
    width: 220,
  },
  {
    title: '操作',
    key: 'operation',
    width: 120,
  },
]

const formatDate = (dateString: string | null): string => {
  return dateString ? dayjs(dateString).format('YYYY-MM-DD HH:mm:ss') : '-'
}

const enterBucket = async (bucket: BucketInfo): Promise<void> => {
  await router.push({
    name: 'FileManager',
    params: {
      id: configId,
      bucket: bucket.name,
    },
  })
}

const goBack = async (): Promise<void> => {
  await router.push({name: 'ConfigList'})
}

const loadData = async (): Promise<void> => {
  const requestVersion = ++loadVersion
  loading.value = true
  try {
    const configs = await configApi.getConfig()
    if (requestVersion !== loadVersion) return
    config.value = configs.find((item) => item.id === configId) || null
    if (!config.value) {
      message.error('未找到配置信息')
      return
    }
    const buckets = await fileApi.listBuckets(configId)
    if (requestVersion !== loadVersion) return
    bucketList.value = buckets
  } catch (error) {
    if (requestVersion !== loadVersion) return
    console.error('加载桶列表失败:', error)
    message.error('加载桶列表失败！请确认当前账号具备 ListBuckets 权限')
  } finally {
    if (requestVersion === loadVersion) {
      loading.value = false
    }
  }
}

const cancelLoad = (): void => {
  loadVersion++
  loading.value = false
  message.info('已停止等待')
}

onMounted(loadData)
</script>

<template>
  <div class="bucket-list-page page-layout">
    <a-layout-header class="header header-layout with-padding">
      <div class="header-left">
        <a-button @click="goBack" :icon="h(ArrowLeftOutlined)" title="返回配置列表"/>
        <span class="title">{{ config?.name || '桶列表' }}</span>
      </div>
      <div class="header-right">
        <a-button v-if="loading" @click="cancelLoad">停止</a-button>
        <a-button @click="loadData" :loading="loading" :icon="h(ReloadOutlined)">刷新</a-button>
      </div>
    </a-layout-header>

    <a-layout-content class="content content-layout">
      <div class="table-card table-card-style">
        <a-table
            :data-source="bucketList"
            :columns="tableColumns"
            :pagination="false"
            :loading="loading"
            bordered
            row-key="name"
            size="middle"
            :scroll="{ y: 'calc(100vh - 165px)' }"
            :customRow="(record: BucketInfo) => ({
              onDblclick: () => enterBucket(record),
            })"
        >
          <template #bodyCell="{ column, record }">
            <template v-if="column.key === 'creationDate'">
              {{ formatDate(record.creationDate) }}
            </template>
            <template v-else-if="column.key === 'operation'">
              <a-button type="link" size="small" @click="enterBucket(record)">进入</a-button>
            </template>
          </template>
        </a-table>
      </div>
    </a-layout-content>
  </div>
</template>

<style scoped lang="less">
.header-left {
  display: flex;
  align-items: center;
  gap: 12px;
}
</style>
