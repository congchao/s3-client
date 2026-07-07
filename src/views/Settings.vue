<script setup lang="ts">
import {computed, h, reactive, ref, watch} from 'vue'
import {message} from 'ant-design-vue'
import {CloudDownloadOutlined, ReloadOutlined, SaveOutlined, SettingOutlined} from '@ant-design/icons-vue'
import {getVersion} from '@tauri-apps/api/app'
import type {AppSettings, AppUpdateCheckResult, ContextMenuSettings} from '@/types'
import {configApi} from '@/services/config.ts'

const props = defineProps<{
  open: boolean
}>()

const emit = defineEmits<{
  (e: 'update:open', value: boolean): void
  (e: 'saved'): void
}>()

const loading = ref<boolean>(false)
const saving = ref<boolean>(false)
const checkingUpdate = ref<boolean>(false)
const installingUpdate = ref<boolean>(false)
const currentVersion = ref<string>('')
const updateInfo = ref<AppUpdateCheckResult | null>(null)
const updateError = ref<string>('')

const visible = computed<boolean>({
  get: () => props.open,
  set: (value) => emit('update:open', value),
})

const defaultMenuSettings = (): ContextMenuSettings => ({
  download: true,
  rename: true,
  moveItem: true,
  duplicate: true,
  share: true,
  delete: true,
  copyPath: true,
  parquetToExcel: true,
})

const settings = reactive<AppSettings>({
  fileContextMenu: defaultMenuSettings(),
  directoryContextMenu: defaultMenuSettings(),
})

const fileMenuItems: Array<{ key: keyof ContextMenuSettings; label: string }> = [
  {key: 'download', label: '下载'},
  {key: 'rename', label: '重命名'},
  {key: 'moveItem', label: '移动'},
  {key: 'duplicate', label: '复制对象'},
  {key: 'share', label: '预签名链接'},
  {key: 'delete', label: '删除'},
  {key: 'copyPath', label: '复制路径'},
  {key: 'parquetToExcel', label: 'Parquet 转 Excel 下载'},
]

const directoryMenuItems: Array<{ key: keyof ContextMenuSettings; label: string }> = [
  {key: 'download', label: '下载'},
  {key: 'rename', label: '重命名'},
  {key: 'moveItem', label: '移动'},
  {key: 'duplicate', label: '复制对象'},
  {key: 'delete', label: '删除'},
  {key: 'copyPath', label: '复制路径'},
  {key: 'parquetToExcel', label: 'Parquet 转 Excel 下载'},
]

const loadSettings = async (): Promise<void> => {
  loading.value = true
  try {
    const result = await configApi.getSettings()
    Object.assign(settings.fileContextMenu, result.fileContextMenu)
    Object.assign(settings.directoryContextMenu, result.directoryContextMenu)
  } catch (error) {
    console.error('加载设置失败:', error)
    message.error('加载设置失败')
  } finally {
    loading.value = false
  }
}

const loadUpdateInfo = async (): Promise<void> => {
  checkingUpdate.value = true
  updateError.value = ''
  try {
    currentVersion.value = await getVersion()
    updateInfo.value = await configApi.checkUpdate(false)
  } catch (error) {
    console.error('检查更新失败:', error)
    updateInfo.value = null
    updateError.value = formatError(error)
  } finally {
    checkingUpdate.value = false
  }
}

const installLatestUpdate = async (): Promise<void> => {
  if (!updateInfo.value?.updateAvailable || installingUpdate.value) {
    return
  }

  installingUpdate.value = true
  const messageKey = 'settings-update-install'
  message.loading({content: '正在下载并安装更新...', key: messageKey, duration: 0})
  try {
    await configApi.installUpdate()
  } catch (error) {
    console.error('更新安装失败:', error)
    message.error({
      content: `更新安装失败：${formatError(error)}`,
      key: messageKey,
      duration: 6,
    })
  } finally {
    installingUpdate.value = false
  }
}

const saveSettings = async (): Promise<void> => {
  saving.value = true
  try {
    await configApi.saveSettings({
      fileContextMenu: {...settings.fileContextMenu},
      directoryContextMenu: {...settings.directoryContextMenu},
    })
    message.success('设置已保存')
    emit('saved')
  } catch (error) {
    console.error('保存设置失败:', error)
    message.error('保存设置失败')
  } finally {
    saving.value = false
  }
}

const formatError = (error: unknown): string => {
  if (error instanceof Error) {
    return error.message
  }
  return String(error)
}

watch(
    () => props.open,
    (isOpen) => {
      if (isOpen) {
        void loadSettings()
        void loadUpdateInfo()
      }
    },
    {immediate: true}
)
</script>

<template>
  <a-modal
      v-model:open="visible"
      width="900px"
      :footer="null"
      destroy-on-close
      wrap-class-name="settings-modal-wrap"
  >
    <template #title>
      <span class="settings-title">
        <SettingOutlined/>
        系统设置
      </span>
    </template>

    <div class="settings-body">
      <a-spin :spinning="loading">
        <div class="settings-section">
          <h2>版本信息</h2>
          <div class="version-panel">
            <div class="version-items">
              <div class="version-item">
                <span class="version-label">当前版本</span>
                <strong>{{ updateInfo?.currentVersion || currentVersion || '-' }}</strong>
              </div>
              <div class="version-item">
                <span class="version-label">最新版本</span>
                <strong>{{ checkingUpdate ? '检查中...' : (updateInfo?.latestTag || updateInfo?.latestVersion || (updateError ? '检查失败' : '-')) }}</strong>
              </div>
            </div>
            <div class="version-actions">
              <a-button :icon="h(ReloadOutlined)" :loading="checkingUpdate" @click="loadUpdateInfo">重新检查</a-button>
              <a-button
                  v-if="updateInfo?.updateAvailable"
                  type="primary"
                  :icon="h(CloudDownloadOutlined)"
                  :loading="installingUpdate"
                  @click="installLatestUpdate"
              >
                更新版本
              </a-button>
            </div>
          </div>
        </div>

        <div class="settings-section">
          <h2>文件右键菜单</h2>
          <div class="settings-grid">
            <div v-for="item in fileMenuItems" :key="`file-${item.key}`" class="settings-row">
              <span>{{ item.label }}</span>
              <a-switch v-model:checked="settings.fileContextMenu[item.key]"/>
            </div>
          </div>
        </div>

        <div class="settings-section">
          <h2>目录右键菜单</h2>
          <div class="settings-grid">
            <div v-for="item in directoryMenuItems" :key="`directory-${item.key}`" class="settings-row">
              <span>{{ item.label }}</span>
              <a-switch v-model:checked="settings.directoryContextMenu[item.key]"/>
            </div>
          </div>
        </div>
      </a-spin>
    </div>

    <div class="settings-footer">
      <a-button @click="visible = false">取消</a-button>
      <a-button type="primary" :loading="saving" :icon="h(SaveOutlined)" @click="saveSettings">保存设置</a-button>
    </div>
  </a-modal>
</template>

<style scoped lang="less">
.settings-title {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

:global(.settings-modal-wrap .ant-modal) {
  top: 48px;
  padding-bottom: 48px;
}

:global(.settings-modal-wrap .ant-modal-content) {
  max-height: calc(100vh - 96px);
  display: flex;
  flex-direction: column;
}

:global(.settings-modal-wrap .ant-modal-body) {
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.settings-body {
  min-height: 0;
  max-height: calc(100vh - 230px);
  overflow-y: auto;
  padding-right: 4px;
}

.settings-section {
  padding: 8px 0 20px;

  & + & {
    border-top: 1px solid #f0f0f0;
    padding-top: 20px;
  }

  h2 {
    margin: 0 0 16px;
    font-size: 16px;
    font-weight: 600;
  }
}

.settings-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
  gap: 12px;
}

.version-panel {
  min-height: 72px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 14px 16px;
  border: 1px solid #f0f0f0;
  border-radius: 6px;
  background: #fff;
}

.version-items {
  min-width: 0;
  display: grid;
  grid-template-columns: repeat(2, minmax(120px, 1fr));
  gap: 16px;
  flex: 1;
}

.version-item {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;

  strong {
    font-size: 15px;
    font-weight: 600;
    color: #1f2937;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
}

.version-label {
  font-size: 12px;
  color: #8c8c8c;
}

.version-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
  flex-shrink: 0;
}

.settings-row {
  min-height: 40px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 0 12px;
  border: 1px solid #f0f0f0;
  border-radius: 6px;
  background: #fff;
}

.settings-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding-top: 16px;
  border-top: 1px solid #f0f0f0;
}

@media (max-width: 760px) {
  :global(.settings-modal-wrap .ant-modal) {
    top: 24px;
    padding-bottom: 24px;
  }

  :global(.settings-modal-wrap .ant-modal-content) {
    max-height: calc(100vh - 48px);
  }

  .settings-body {
    max-height: calc(100vh - 206px);
  }

  .version-panel {
    align-items: stretch;
    flex-direction: column;
  }

  .version-items {
    grid-template-columns: 1fr;
  }

  .version-actions {
    justify-content: flex-start;
    flex-wrap: wrap;
  }
}
</style>
