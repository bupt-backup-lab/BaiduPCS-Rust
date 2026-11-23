import { ElMessage, ElMessageBox } from 'element-plus'
import { useOpenListStore } from '@/stores/openlist'
import { useAuthStore } from '@/stores/auth'
import { executeOpenListOpBackend } from '@/api/config'
import { resolveAccountMount } from '@/utils/openlist'
import type { FileOperationItem, RenameItem } from '@/api/file'

export interface TakeoverParams {
  item?: RenameItem
  paths?: string[]
  items?: FileOperationItem[]
  ondup?: string
}

/**
 * 统一处理文件操作的 OpenList 接管逻辑
 *
 * @param op 操作类型: 'rename' | 'delete' | 'move' | 'copy'
 * @param params 操作参数
 * @param nativeFn BaiduPCS 原生接口执行函数
 */
export async function handleTakeoverOperation<T>(
  op: 'rename' | 'delete' | 'move' | 'copy',
  params: TakeoverParams,
  nativeFn: () => Promise<T>
): Promise<T> {
  let openListStore: ReturnType<typeof useOpenListStore> | null = null
  let authStore: ReturnType<typeof useAuthStore> | null = null

  try {
    openListStore = useOpenListStore()
    authStore = useAuthStore()
  } catch {
    // 若 Pinia 尚未初始化，直接走原生
    return nativeFn()
  }

  // 1. 未启用 OpenList 或模式为 "off"（关闭）时直接走原生
  if (!openListStore.config.enabled) {
    return nativeFn()
  }

  const mode = openListStore.config.takeoverMode || 'off'
  if (mode === 'off') {
    return nativeFn()
  }

  // 2. 检查当前账号是否已配置 OpenList 挂载点
  const activeAccount = authStore.activeAccount
  const activeUid = authStore.activeUid
  const resolved = resolveAccountMount(
    activeAccount || (activeUid ? { uid: activeUid } : null),
    openListStore.config
  )

  if (resolved.error) {
    if (mode === 'observe') {
      ElMessage.info(
        `这次「${op}」操作无法交由 OpenList（${resolved.error}），将由 BaiduPCS 原生执行`
      )
    }
    return nativeFn()
  }

  // 3. 构建操作描述，并针对无法接管的情况做前置校验
  let desc = ''
  if (op === 'rename') {
    const item = params.item
    if (!item || !item.path) return nativeFn()
    const oldName = item.path.split('/').filter(Boolean).pop() || ''
    desc = `${resolved.label}：重命名「${oldName}」→「${item.newname}」`
  } else if (op === 'delete') {
    const paths = params.paths
    if (!paths || paths.length === 0) return nativeFn()
    const names = paths.map((p) => p.split('/').filter(Boolean).pop() || '')
    const preview = names.slice(0, 3).join('、') + (names.length > 3 ? '…' : '')
    desc = `${resolved.label}：删除 ${names.length} 项（${preview}）`
  } else {
    const items = params.items
    if (!items || items.length === 0) return nativeFn()

    // 检查是否存在移动/复制的同时重命名（OpenList 无法一步完成，退回原生）
    const hasRename = items.some((it) => {
      const base = it.path.split('/').filter(Boolean).pop() || ''
      return it.newname && it.newname !== base
    })
    if (hasRename) {
      if (mode === 'observe') {
        ElMessage.info('移动/复制的同时重命名，OpenList 无法一步完成，将由 BaiduPCS 原生执行')
      }
      return nativeFn()
    }

    const opName = op === 'move' ? '移动' : '复制'
    const names = items.map((it) => it.path.split('/').filter(Boolean).pop() || '')
    const preview = names.slice(0, 3).join('、') + (names.length > 3 ? '…' : '')
    desc = `${resolved.label}：${opName} ${items.length} 项（${preview}）→ ${items[0].dest}`
  }

  // 4. 观察模式（observe）：只弹提示，仍走原生
  if (mode === 'observe') {
    ElMessage({
      type: 'warning',
      message: `👀 观察模式：这次操作可以交由 OpenList 执行：\n${desc}\n仍由 BaiduPCS 原生执行；可在顶栏或设置中切换为“接管模式”`,
      duration: 6000,
    })
    return nativeFn()
  }

  // 5. 接管模式（on）：交由 OpenList 执行
  try {
    ElMessage.info({
      message: `正在通过 OpenList 执行：\n${desc}`,
      duration: 10000,
    })

    const res = await executeOpenListOpBackend({
      op,
      uid: activeUid ?? undefined,
      username: activeAccount?.username || undefined,
      path: params.item?.path,
      new_name: params.item?.newname,
      paths: params.paths,
      items: params.items,
    })

    ElMessage.success(`✅ 已通过 OpenList 完成：\n${res.desc || desc}`)
    return res.data as T
  } catch (err: any) {
    const errMsg = err?.response?.data?.message || err?.message || '未知错误'

    // OpenList 失败时弹窗询问是否改用原生方式执行
    try {
      await ElMessageBox.confirm(
        `OpenList 执行失败：\n${errMsg}\n\n是否改用 BaiduPCS 原生方式执行这次操作？`,
        'OpenList 操作接管失败',
        {
          confirmButtonText: '改用原生执行',
          cancelButtonText: '取消操作',
          type: 'warning',
          distinguishCancelAndClose: true,
        }
      )
      ElMessage.info('已改用 BaiduPCS 原生方式执行')
      return nativeFn()
    } catch {
      ElMessage.warning('操作已取消')
      throw new Error(`OpenList 操作已取消: ${errMsg}`)
    }
  }
}
