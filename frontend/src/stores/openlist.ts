import { defineStore } from 'pinia'
import { ref } from 'vue'
import { ElMessage } from 'element-plus'
import type { OpenListConfig, OpenListTakeoverMode } from '@/types/openlist'
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
  takeoverMode: 'observe',
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

  function setTakeoverMode(mode: OpenListTakeoverMode) {
    config.value.takeoverMode = mode
    return saveConfig({ takeoverMode: mode })
  }

  function cycleTakeoverMode(): OpenListTakeoverMode {
    const order: OpenListTakeoverMode[] = ['off', 'observe', 'on']
    const current = config.value.takeoverMode || 'off'
    const next = order[(order.indexOf(current) + 1) % 3]
    config.value.takeoverMode = next
    void saveConfig({ takeoverMode: next })
    const labelMap: Record<OpenListTakeoverMode, string> = {
      off: '关闭',
      observe: '观察模式',
      on: '接管模式',
    }
    ElMessage({
      type: next === 'on' ? 'success' : next === 'observe' ? 'warning' : 'info',
      message: `OpenList 接管：${labelMap[next]}${
        next === 'on'
          ? '\n同网盘内的 重命名/删除/移动/复制 改由 OpenList 执行'
          : next === 'observe'
          ? '\n只提示，仍由 BaiduPCS 原生执行'
          : '\n全部走 BaiduPCS 原生接口'
      }`,
      duration: 3500,
    })
    return next
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
    setTakeoverMode,
    cycleTakeoverMode,
  }
})
