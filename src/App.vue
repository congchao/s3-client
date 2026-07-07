<template>
  <div class="app-container">
    <router-view />
    <Settings v-model:open="settingsOpen" @saved="handleSettingsSaved"/>
  </div>
</template>

<script setup lang="ts">
import {onMounted, onUnmounted, ref} from 'vue'
import {listen} from '@tauri-apps/api/event'
import {Modal, message} from 'ant-design-vue'
import Settings from '@/views/Settings.vue'
import {configApi} from '@/services/config'
import type {AppUpdateCheckResult} from '@/types'

let unlistenOpenSettings: (() => void) | null = null
let unlistenCheckUpdate: (() => void) | null = null
const settingsOpen = ref<boolean>(false)
const checkingUpdate = ref<boolean>(false)
const installingUpdate = ref<boolean>(false)

onMounted(async () => {
  unlistenOpenSettings = await listen('open_settings', () => {
    settingsOpen.value = true
  })
  unlistenCheckUpdate = await listen('check_update', () => {
    void checkForUpdates(true)
  })
  void checkForUpdates(false)
})

onUnmounted(() => {
  unlistenOpenSettings?.()
  unlistenCheckUpdate?.()
})

const handleSettingsSaved = (): void => {
  window.dispatchEvent(new CustomEvent('app-settings-saved'))
}

const checkForUpdates = async (interactive: boolean): Promise<void> => {
  if (checkingUpdate.value || installingUpdate.value) {
    return
  }

  checkingUpdate.value = true
  const messageKey = 'app-update-check'
  if (interactive) {
    message.loading({content: '正在检查更新...', key: messageKey, duration: 0})
  }

  try {
    const result = await configApi.checkUpdate(interactive)
    if (interactive) {
      message.destroy(messageKey)
    }
    showUpdateResult(result, interactive)
  } catch (error) {
    if (interactive) {
      message.error({
        content: `检查更新失败：${formatError(error)}`,
        key: messageKey,
        duration: 4
      })
    } else {
      console.warn('检查更新失败', error)
    }
  } finally {
    checkingUpdate.value = false
  }
}

const showUpdateResult = (result: AppUpdateCheckResult, interactive: boolean): void => {
  if (result.shouldPrompt) {
    Modal.confirm({
      title: '发现新版本',
      content: `最新版本：${result.latestTag || result.latestVersion}，当前版本：${result.currentVersion}。是否立即升级？`,
      okText: '立即升级',
      cancelText: '跳过此版本',
      centered: true,
      async onOk() {
        await installLatestUpdate()
      },
      async onCancel() {
        await configApi.skipUpdate(result.latestVersion)
        message.info(`已跳过版本 ${result.latestTag || result.latestVersion}`)
      }
    })
    return
  }

  if (!interactive) {
    return
  }

  if (result.updateAvailable && result.skipped) {
    message.info(`最新版本 ${result.latestTag || result.latestVersion} 已被跳过`)
    return
  }

  message.success(`当前已是最新版本：${result.currentVersion}`)
}

const installLatestUpdate = async (): Promise<void> => {
  if (installingUpdate.value) {
    return
  }

  installingUpdate.value = true
  const messageKey = 'app-update-install'
  message.loading({content: '正在下载并安装更新...', key: messageKey, duration: 0})
  try {
    await configApi.installUpdate()
  } catch (error) {
    message.error({
      content: `更新安装失败：${formatError(error)}`,
      key: messageKey,
      duration: 6
    })
    throw error
  } finally {
    installingUpdate.value = false
  }
}

const formatError = (error: unknown): string => {
  if (error instanceof Error) {
    return error.message
  }
  return String(error)
}
</script>

<style scoped>
.app-container {
  height: 100vh;
  display: flex;
  flex-direction: column;
}
</style>
