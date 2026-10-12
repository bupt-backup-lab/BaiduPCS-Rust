import { defineStore } from 'pinia'
import { ref } from 'vue'
import { getConfig, updateConfig, type AppConfig } from '@/api/config'

export const useConfigStore = defineStore('config', () => {
  // 状态
  const config = ref<AppConfig | null>(null)
  const loading = ref(false)

  // 获取配置
  async function fetchConfig() {
    loading.value = true
    try {
      config.value = await getConfig()
      return config.value
    } catch (error) {
      console.error('获取配置失败:', error)
      throw error
    } finally {
      loading.value = false
    }
  }

  // 更新配置
  async function saveConfig(newConfig: AppConfig) {
    loading.value = true
    try {
      await updateConfig(newConfig)
      // 存快照而非引用：调用方（设置页表单）后续未保存的编辑不应提前生效
      config.value = JSON.parse(JSON.stringify(newConfig))
    } catch (error) {
      console.error('更新配置失败:', error)
      throw error
    } finally {
      loading.value = false
    }
  }

  return {
    // 状态
    config,
    loading,

    // 方法
    fetchConfig,
    saveConfig,
  }
})

