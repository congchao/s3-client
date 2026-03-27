<script setup lang="ts">
import {h, onMounted, ref} from 'vue'
import {useRouter} from 'vue-router'
import {message, Modal} from 'ant-design-vue'
import {PlusOutlined} from '@ant-design/icons-vue'
import {OssConfig} from '@/types/index.d'
import {configApi} from '@/services/config.ts'

const router = useRouter()
const loading = ref<boolean>(false)
const configList = ref<OssConfig[]>([])

// 表格列配置
// 表格列配置 - 添加缺失的字段
const tableColumns = [
  {
    title: '配置名称',
    dataIndex: 'name',
    key: 'name',
    ellipsis: true,
  },
  {
    title: '提供商',
    key: 'provider',
    width: 100,
    ellipsis: true,
  },
  {
    title: '端点',
    dataIndex: 'endpoint',
    key: 'endpoint',
    ellipsis: true,
    width: 200,
  },
  {
    title: '存储桶',
    dataIndex: 'bucket',
    key: 'bucket',
    width: 150,
    ellipsis: true,
  },
  {
    title: '操作',
    key: 'operation',
    width: 220,
  },
]

// 加载配置列表
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
      pathStyle: config.pathStyle
    } as OssConfig))
  } catch (error) {
    console.error('加载配置列表失败:', error)
    message.error('加载配置列表失败！')
  } finally {
    loading.value = false
  }
}

// 新增配置
const addConfig = (): void => {
  router.push({name: 'NewConfig'})
}

// 编辑配置
const editConfig = (record: OssConfig): void => {
  router.push({name: 'EditConfig', params: {id: record.id}})
}

// 删除配置
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

// 测试单个配置
// 测试单个配置
const testSingleConfig = async (record: OssConfig): Promise<void> => {
  try {
    message.loading('测试中...', 0) // 显示持续加载状态

    // 获取完整配置信息
    const configs: OssConfig[] = await configApi.getConfig()
    const config = configs.find(c => c.id === record.id)

    if (!config) {
      message.destroy() // 清除加载提示
      message.error('未找到配置信息')
      return
    }

    // 调用测试连接命令
    const result: boolean = await configApi.testConfig(config)

    message.destroy() // 清除加载提示

    if (result) {
      message.success(`【${record.name}】连接测试成功！`)
    } else {
      message.error(`【${record.name}】连接测试失败`)
    }
  } catch (error) {
    console.error('测试单个配置失败:', error)
    message.destroy() // 清除加载提示
    message.error(`测试失败: ${error instanceof Error ? error.message : '未知错误'}`)
  }
}

// 在操作按钮中添加进入文件管理的按钮
const enterFileManager = (record: OssConfig): void => {
  router.push({
    name: 'FileManager',
    params: {id: record.id}
  })
}

// 页面初始化
onMounted(async () => {
  await loadConfigList()
})
</script>
<template>
  <div class="config-list-page page-layout">
    <!-- 顶部导航栏 -->
    <a-layout-header class="header header-layout with-padding">
      <div class="header-left">
        <span class="title">OSS管理工具 - 配置列表</span>
      </div>
      <div class="header-right">
        <a-button type="primary" @click="addConfig" :icon="h(PlusOutlined)">新增配置</a-button>
      </div>
    </a-layout-header>

    <!-- 主体内容区 -->
    <a-layout-content class="content content-layout">
      <div class="table-card table-card-style">
        <a-table
            :data-source="configList"
            bordered
            :pagination="false"
            :columns="tableColumns"
            row-key="id"
            :loading="loading"            style="width: 100%;"
            size="middle"
            :scroll="{ y: 'calc(100vh - 165px)' }"
            :customRow="(record:OssConfig) => ({
              onDblclick: () => {
                enterFileManager(record)
              }
            })"
        >
          <!-- 自定义单元格渲染 -->
          <template #bodyCell="{ column, record }">
            <template v-if="column.key === 'provider'">
              <a-tag>{{ record.provider }}</a-tag>
            </template>
            <template v-else-if="column.key === 'operation'">
              <a-space wrap>
                <a-button type="link" size="small" @click="editConfig(record)">编辑</a-button>
                <a-button type="link" size="small" @click="testSingleConfig(record)">测试</a-button>
                <a-button type="link" size="small" @click="enterFileManager(record)">管理</a-button>
                <a-button type="link" danger size="small" @click="del(record)">删除</a-button>
              </a-space>
            </template>
          </template>
        </a-table>
      </div>
    </a-layout-content>
  </div>
</template>
<style scoped lang="less">

</style>
