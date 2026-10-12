import { createApiClient } from './client'

// 错误由调用方处理（不支持时回退为展示路径），不弹全局错误提示
const apiClient = createApiClient({ timeout: 10000, showErrorMessage: false })

/// 打开本地文件夹能力
export interface OpenFolderCapability {
  /** 是否可直接唤起系统文件管理器（后端在桌面系统上运行且浏览器在本机访问） */
  supported: boolean
  /** 不支持时的原因 */
  reason?: string
}

let capabilityPromise: Promise<OpenFolderCapability> | null = null

/**
 * 查询能否直接打开本地文件夹（结果在页面生命周期内缓存；请求失败视为不支持且下次重试）
 */
export function getOpenFolderCapability(): Promise<OpenFolderCapability> {
  if (!capabilityPromise) {
    capabilityPromise = (apiClient.get('/system/open-folder') as Promise<OpenFolderCapability>)
        .catch((error: Error) => {
          capabilityPromise = null
          return { supported: false, reason: error.message }
        })
  }
  return capabilityPromise
}

/**
 * 在运行后端的电脑上用系统文件管理器打开路径（文件则打开所在目录并选中）
 */
export async function openLocalFolder(path: string): Promise<void> {
  await apiClient.post('/system/open-folder', { path })
}
