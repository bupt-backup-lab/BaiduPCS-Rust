import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { OpenListConfig } from '@/types/openlist'
import { getOpenListConfig, updateOpenListConfig } from '@/api/config'

const STORAGE_KEY = 'pcs_openlist_config'

const DEFAULT_CONFIG: OpenListConfig = {
  enabled: false,
  baseUrl: '',
  username: '',
  password: '',
  hasPassword: false,
  desktopScheme: 'mpvpcs',
  autoMountByUsername: true,
  fallbackMount: '',
  accounts: {},
}

export const useOpenListStore = defineStore('openList', () => {
  const config = ref<OpenListConfig>({ ...DEFAULT_CONFIG })
  const loading = ref(false)

  /**
   * 从后端加载 OpenList 配置（从 app.toml 读取）
   */
  async function loadConfig() {
    loading.value = true
    try {
      const remote = await getOpenListConfig()
      if (remote) {
        config.value = {
          ...DEFAULT_CONFIG,
          ...remote,
          accounts: remote.accounts || {},
        }
        // 缓存到本地备用
        try {
          localStorage.setItem(STORAGE_KEY, JSON.stringify(config.value))
        } catch {}
        return
      }
    } catch (e) {
      console.warn('从后端加载 OpenList 配置失败，尝试从本地缓存读取', e)
    } finally {
      loading.value = false
    }

    // 降级：从本地缓存尝试读取
    try {
      const saved = localStorage.getItem(STORAGE_KEY)
      if (saved) {
        const parsed = JSON.parse(saved)
        config.value = {
          ...DEFAULT_CONFIG,
          ...parsed,
          accounts: parsed.accounts || {},
        }
      }
    } catch {}
  }

  /**
   * 保存 OpenList 配置（原子写入后端 app.toml）
   */
  async function saveConfig(newConfig?: Partial<OpenListConfig>) {
    if (newConfig) {
      config.value = {
        ...config.value,
        ...newConfig,
      }
    }

    try {
      // 写入后端 app.toml
      const saved = await updateOpenListConfig(config.value)
      if (saved) {
        config.value = {
          ...config.value,
          ...saved,
          accounts: saved.accounts || {},
        }
      }
      try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(config.value))
      } catch {}
    } catch (e) {
      console.error('保存 OpenList 配置到服务端失败', e)
      throw e
    }
  }

  function setAccountMapping(accountKey: string, mount: string, root = '/') {
    config.value.accounts = {
      ...config.value.accounts,
      [accountKey]: {
        mount: mount.trim(),
        root: root.trim() || '/',
      },
    }
    return saveConfig()
  }

  function removeAccountMapping(accountKey: string) {
    const updated = { ...config.value.accounts }
    delete updated[accountKey]
    config.value.accounts = updated
    return saveConfig()
  }

  // 初始自动从后端加载配置
  void loadConfig()

  return {
    config,
    loading,
    loadConfig,
    saveConfig,
    setAccountMapping,
    removeAccountMapping,
  }
})
