<script setup lang="ts">
import {h, onMounted, reactive, ref} from 'vue'
import {useRoute, useRouter} from 'vue-router'
import {message, Modal} from 'ant-design-vue'
import {ArrowLeftOutlined, CheckOutlined, SyncOutlined} from '@ant-design/icons-vue'
import {configApi} from '@/services/config.ts'
import {OssConfig} from '@/types/index.d'
import {PROVIDER_OPTIONS, REGION_OPTIONS} from '@/types/constants.ts'


// 获取路由参数
const route = useRoute()
const router = useRouter()
const basicFormRef = ref()
const testing = ref<boolean>(false)

// 基础表单数据
const basicForm = reactive<OssConfig>({
  id: route.params.id as string,
  name: '',
  provider: '',
  region: '',
  accessKey: '',
  secretKey: '',
  endpoint: '',
  bucket: '',
  pathStyle: 'path',
})

// 表单校验规则
const basicRules = reactive({
  name: [{required: true, message: '请输入连接名称', trigger: 'blur'}],
  provider: [{required: true, message: '请选择存储提供商', trigger: 'change'}],
  region: [{required: true, message: '请选择存储区域', trigger: 'change'}],
  accessKey: [{required: true, message: '请输入Access Key ID', trigger: 'blur'}],
  secretKey: [{required: true, message: '请输入Secret Access Key', trigger: 'blur'}],
  endpoint: [
    {required: true, message: '请输入端点URL', trigger: 'blur'},
    {type: 'url', message: '请输入合法的URL', trigger: 'blur'},
  ],
  bucket: [{required: true, message: '请输入存储桶名称', trigger: 'blur'}],
})

// 获取当前是编辑还是新增
const isEdit = ref<boolean>(!!route.params.name)

// 返回上一页
const goBack = (): void => {
  // 未保存校验
  // 检查是否有任何关键字段被填写
  const hasUnsavedChanges = basicForm.name.trim() !== '' ||
      basicForm.provider.trim() !== '' ||
      basicForm.region.trim() !== '' ||
      basicForm.accessKey.trim() !== '' ||
      basicForm.secretKey.trim() !== '' ||
      basicForm.endpoint.trim() !== '' ||
      basicForm.bucket.trim() !== ''

  if (hasUnsavedChanges) {
    Modal.confirm({
      title: '提示',
      content: '配置未保存，确定返回吗？',
      okText: '确定',
      cancelText: '取消',
      onOk: () => {
        router.push({name: 'ConfigList'})
      },
    })
  } else {
    router.push({name: 'ConfigList'})
  }
}

// 测试连接
const testConnection = async (): Promise<void> => {
  try {
    await basicFormRef.value.validate()
    testing.value = true

    // 构建配置对象用于测试
    const config: OssConfig = {
      id: route.params.name as string,
      name: basicForm.name || 'test-connection',
      provider: basicForm.provider,
      region: basicForm.region,
      accessKey: basicForm.accessKey,
      secretKey: basicForm.secretKey,
      endpoint: basicForm.endpoint,
      bucket: basicForm.bucket,
      pathStyle: basicForm.pathStyle,
    }

    // 调用后端测试连接命令
    const result: boolean = await configApi.testConfig(config)

    if (result) {
      message.success('连接测试成功！')
    } else {
      message.error('连接测试失败，请检查配置信息')
    }
  } catch (error) {
    console.error('连接测试失败:', error)
    message.error(`连接测试失败: ${error instanceof Error ? error.message : '未知错误'}`)
  } finally {
    testing.value = false
  }
}
// 保存配置
const save = async (): Promise<void> => {
  try {
    await basicFormRef.value.validate()

    // 直接传递驼峰式命名的配置给后端
    const config: OssConfig = {
      id: route.params.id as string || crypto.randomUUID(),
      name: basicForm.name,
      provider: basicForm.provider,
      region: basicForm.region,
      accessKey: basicForm.accessKey,
      secretKey: basicForm.secretKey,
      endpoint: basicForm.endpoint,
      bucket: basicForm.bucket,
      pathStyle: basicForm.pathStyle,
    }

    // 调用后端命令保存配置
    await configApi.saveConfig(config)
    message.success('配置保存成功！')

    // 返回配置列表页
    await router.push({name: 'ConfigList'})
  } catch (error) {
    console.error('保存配置失败:', error)
    message.error('配置保存失败！')
  }
}

// 初始化编辑模式数据
onMounted(async () => {
  if (route.params.id) {
    // 编辑模式，获取现有配置
    try {
      const configs: OssConfig[] = await configApi.getConfig()
      const config = configs.find(c => c.id === route.params.id) as OssConfig

      if (config) {
        Object.assign(basicForm, {
          id: config.id,  // 保存现有id
          name: config.name,
          provider: config.provider,
          region: config.region,
          accessKey: config.accessKey,
          secretKey: config.secretKey,
          endpoint: config.endpoint,
          bucket: config.bucket,
          pathStyle: config.pathStyle || 'path',
        })
      }
    } catch (error) {
      console.error('获取配置失败:', error)
      message.error('获取配置失败！')
    }
  } else {
    // 新增模式，生成新ID
    basicForm.id = crypto.randomUUID();
  }
})
</script>

<template>
  <div class="config-form-page">
    <!-- 顶部导航栏 -->
    <a-layout-header class="header header-layout">
      <div class="header-left">
        <span class="title">{{ isEdit ? '编辑配置' : '新增配置' }}</span>
      </div>
      <div class="header-right">
        <a-button @click="goBack" :icon="h(ArrowLeftOutlined)">返回</a-button>
      </div>
    </a-layout-header>

    <!-- 主体内容区 -->
    <a-layout-content class="content content-layout">
      <div class="form-card card-style">
        <a-form
            ref="basicFormRef"
            :model="basicForm"
            :rules="basicRules"
            :label-col="{ span: 4 }"
            :wrapper-col="{ span: 20 }"
            layout="horizontal">
          <a-form-item label="连接名称" name="name">
            <a-input v-model:value="basicForm.name" placeholder="请输入连接名称" :maxlength="50"/>
          </a-form-item>
          <a-form-item label="存储提供商" name="provider">
            <a-auto-complete
                v-model:value="basicForm.provider"
                :options="PROVIDER_OPTIONS"
                placeholder="请输入或选择存储提供商"
                :filter-option="true"
            />
          </a-form-item>
          <a-form-item label="区域" name="region">
            <a-auto-complete
                v-model:value="basicForm.region"
                :options="REGION_OPTIONS"
                placeholder="请输入或选择存储区域"
                :filter-option="true"
            />
          </a-form-item>
          <a-form-item label="Access Key ID" name="accessKey">
            <a-input v-model:value="basicForm.accessKey" placeholder="请输入Access Key ID"/>
          </a-form-item>
          <a-form-item label="Secret Access Key" name="secretKey">
            <a-input-password
                v-model:value="basicForm.secretKey"
                placeholder="请输入Secret Access Key"
                :visibility-toggle="true"
            />
          </a-form-item>
          <a-form-item label="端点URL" name="endpoint">
            <a-input v-model:value="basicForm.endpoint" placeholder="请输入S3兼容端点URL"/>
          </a-form-item>
          <a-form-item label="存储桶名称" name="bucket">
            <a-input v-model:value="basicForm.bucket" placeholder="请输入存储桶名称"/>
          </a-form-item>
          <a-form-item label="路径访问样式" name="pathStyle">
            <a-radio-group v-model:value="basicForm.pathStyle">
              <a-radio value="path">路径样式</a-radio>
              <a-radio value="virtual">虚拟托管</a-radio>
            </a-radio-group>
          </a-form-item>
          <a-form-item>
            <a-flex align="center" justify="center" gap="middle">
              <a-button type="primary" @click="save" :icon="h(CheckOutlined)">保存配置</a-button>
              <a-button @click="testConnection" :loading="testing" :icon="h(SyncOutlined)">测试连接</a-button>
            </a-flex>
          </a-form-item>
        </a-form>
      </div>
    </a-layout-content>
  </div>
</template>

<style scoped lang="less">
.config-form-page {
  height: 100vh;
  display: flex;
  flex-direction: column;
}
</style>
