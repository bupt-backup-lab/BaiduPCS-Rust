import { ElMessage } from 'element-plus'
import type { OpenListAccountMapping, OpenListConfig } from '@/types/openlist'
import { refreshOpenListBackend, testOpenListBackend } from '@/api/config'

export const MEDIA_EXT = /\.(mkv|mp4|avi|ts|m2ts|flv|wmv|mov|rmvb|webm|m4v|mpg|mpeg|mp3|flac|m4a|wav|aac|ogg|opus|ape|wma)$/i
export const VIDEO_EXT = /\.(mkv|mp4|avi|ts|m2ts|flv|wmv|mov|rmvb|webm|m4v|mpg|mpeg)$/i

export function isMediaFile(name: string): boolean {
  return MEDIA_EXT.test(name)
}

export function isVideoFile(name: string): boolean {
  return VIDEO_EXT.test(name)
}

export function splitPath(p: string): string[] {
  return String(p).split('/').filter(Boolean)
}

export function normPath(p: string): string {
  return '/' + splitPath(p).join('/')
}

export interface AccountLike {
  uid: number | string
  username?: string
  nickname?: string | null
}

export interface ResolvedMount {
  label: string
  mount: string
  base: string
  mountParts: string[]
  rootParts: string[]
  error?: string
}

/**
 * 解析百度账号在 OpenList 上的挂载路径及根目录配置
 */
export function resolveAccountMount(
  account: AccountLike | null | undefined,
  config: OpenListConfig
): ResolvedMount {
  const base = config.baseUrl.replace(/\/+$/, '')
  if (!account) {
    if (config.fallbackMount) {
      return {
        label: '默认账号',
        mount: config.fallbackMount,
        base,
        mountParts: splitPath(config.fallbackMount),
        rootParts: [],
      }
    }
    return {
      label: '未识别账号',
      mount: '',
      base,
      mountParts: [],
      rootParts: [],
      error: '未识别到当前百度账号，请先选择账号或在设置中指定挂载路径',
    }
  }

  const label = account.username || account.nickname || String(account.uid)
  const accMap = config.accounts || {}
  const mapping: OpenListAccountMapping | undefined =
    accMap[String(account.uid)] ||
    (account.username ? accMap[account.username] : undefined) ||
    (account.nickname ? accMap[account.nickname] : undefined)

  if (mapping && mapping.mount) {
    const accBase = mapping.base ? mapping.base.replace(/\/+$/, '') : base
    return {
      label,
      mount: mapping.mount,
      base: accBase,
      mountParts: splitPath(mapping.mount),
      rootParts: splitPath(mapping.root || '/'),
    }
  }

  if (config.autoMountByUsername && account.username) {
    const autoMount = '/' + account.username
    return {
      label,
      mount: autoMount,
      base,
      mountParts: splitPath(autoMount),
      rootParts: [],
    }
  }

  return {
    label,
    mount: '',
    base,
    mountParts: [],
    rootParts: [],
    error: `百度账号「${label}」尚未在设置中配置对应的 OpenList 挂载路径`,
  }
}

/**
 * 将网盘路径转换为相对于 OpenList 存储 root 的相对路径
 */
export function getRelativeParts(resolved: ResolvedMount, fullPathParts: string[]): string[] | null {
  for (let i = 0; i < resolved.rootParts.length; i++) {
    if (fullPathParts[i] !== resolved.rootParts[i]) {
      return null
    }
  }
  return fullPathParts.slice(resolved.rootParts.length)
}

/**
 * 生成文件在 OpenList 上的直链下载/播放地址
 */
export function getOpenListDirectUrl(
  account: AccountLike | null | undefined,
  dir: string,
  filename: string,
  config: OpenListConfig
): { url: string | null; error?: string } {
  const resolved = resolveAccountMount(account, config)
  if (resolved.error) return { url: null, error: resolved.error }

  const fullParts = [...splitPath(dir), filename]
  const rel = getRelativeParts(resolved, fullParts)
  if (!rel) {
    return { url: null, error: '当前目录不在该账号 OpenList 挂载的根目录下' }
  }

  const encoded = [...resolved.mountParts, ...rel].map(encodeURIComponent).join('/')
  return { url: `${resolved.base}/d/${encoded}` }
}

/**
 * 生成网盘目录对应的 OpenList Web 浏览地址
 */
export function getOpenListDirUrl(
  account: AccountLike | null | undefined,
  dir: string,
  config: OpenListConfig
): { url: string | null; label?: string; error?: string } {
  const resolved = resolveAccountMount(account, config)
  if (resolved.error) return { url: null, error: resolved.error }

  const rel = getRelativeParts(resolved, splitPath(dir))
  if (!rel) {
    return { url: null, error: '当前目录不在该账号 OpenList 挂载的根目录下' }
  }

  const encoded = [...resolved.mountParts, ...rel].map(encodeURIComponent).join('/')
  return {
    url: `${resolved.base}/${encoded}`,
    label: resolved.label,
  }
}

/**
 * 复制文本到剪贴板并提示
 */
export async function copyDirectUrl(url: string, tip = 'OpenList 直链已复制到剪贴板'): Promise<boolean> {
  try {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      await navigator.clipboard.writeText(url)
      ElMessage.success(tip)
      return true
    }
    const input = document.createElement('input')
    input.value = url
    document.body.appendChild(input)
    input.select()
    const ok = document.execCommand('copy')
    document.body.removeChild(input)
    if (ok) {
      ElMessage.success(tip)
      return true
    }
  } catch (e) {
    console.error('复制直链失败:', e)
  }
  window.prompt('请手动复制链接：', url)
  return false
}

export function formatFileSize(bytes: number): string {
  if (!bytes) return ''
  if (bytes >= 1073741824) return (bytes / 1073741824).toFixed(1) + ' GB'
  return Math.max(1, Math.round(bytes / 1048576)) + ' MB'
}

/**
 * 在浏览器中打开文件（预览或下载）
 */
export function openInBrowser(url: string, size?: number, name?: string): void {
  const sizeText = size ? `（${formatFileSize(size)}）` : ''
  const w = window.open(url, '_blank')
  if (w) {
    ElMessage.success(`已在浏览器新标签页中打开${sizeText}`)
  } else {
    copyDirectUrl(url, '浏览器拦截了新标签页，链接已自动复制，请粘贴到地址栏打开')
  }
}

/**
 * 唤起 MPV 播放器
 */
export function launchMpv(url: string, size?: number, title?: string, desktopScheme = 'mpvpcs'): void {
  const sz = size ? `（${formatFileSize(size)}）` : ''
  const head = title ? `${title}\n` : ''
  const ua = navigator.userAgent

  if (/Android/i.test(ua)) {
    // Android 端：尝试通过 mpv-android intent 唤起
    const intent =
      url.replace(/^https:\/\//i, 'intent://') +
      '#Intent;scheme=https;action=android.intent.action.VIEW;type=video/*;package=is.xyz.mpv;end;'
    ElMessage.info(`${head}正在唤起 MPV Android 播放器${sz}...`)
    window.location.href = intent
    return
  }

  if (/iPhone|iPad|iPod/i.test(ua)) {
    ElMessage.warning('iOS 系统无法直接唤起 MPV，建议点击「复制直链」或「浏览器打开」')
    return
  }

  // 桌面端（Windows / macOS / Linux）：使用注册表自定义协议唤起
  const schemeUrl = url.replace(/^https?:\/\//i, `${desktopScheme}://`)
  ElMessage.info(`${head}正在唤起本地 MPV 播放器${sz}...`)
  window.location.href = schemeUrl
}

/* ============================================================================
 * 剧集智能识别与上下集查找算法
 * ============================================================================ */

function cleanFilename(name: string): string {
  return name
    .replace(VIDEO_EXT, '')
    .replace(/_/g, ' ')
    .replace(/\b(?:19|20)\d{2}[-. ]\d{1,2}[-. ]\d{1,2}\b/g, ' ')
    .replace(/\b\d{3,4}\s*[x×]\s*\d{3,4}\b/gi, ' ')
    .replace(/\b(?:240|360|480|576|720|1080|1440|2160|4320)\s*[pi]\b/gi, ' ')
    .replace(/\b[248]k\b/gi, ' ')
    .replace(/\b[xh][ .-]?26[45]\b/gi, ' ')
    .replace(
      /\b(?:hevc|avc|av1|aac|flac|ac3|eac3|dts|opus|ddp?|atmos|truehd)(?:[ .-]?\d(?:\.\d)?)?\b/gi,
      ' '
    )
    .replace(/\b\d{1,2}[ -]?bits?\b/gi, ' ')
    .replace(/\b\d\.\d\b/g, ' ')
    .replace(/(\d)v\d\b/gi, '$1')
    .replace(/\bv\d\b/gi, ' ')
    .replace(/[\[(][0-9a-f]{8}[\])]/gi, ' ')
    .replace(/\b\d+(?:\.\d+)?\s*(?:gb|mb)\b/gi, ' ')
}

function explicitEp(name: string): number | null {
  const s = cleanFilename(name)
  let m: RegExpMatchArray | null
  if ((m = s.match(/S(\d{1,2})[ ._-]*E(\d{1,4})/i))) return +m[1] * 10000 + +m[2]
  if ((m = s.match(/(?:^|[^A-Za-z0-9])(\d{1,2})x(\d{2,3})(?![A-Za-z0-9])/i)))
    return +m[1] * 10000 + +m[2]
  if ((m = s.match(/第\s*(\d{1,4})\s*[集话話期回章]/))) return +m[1]
  if ((m = s.match(/(?:^|[^A-Za-z0-9])(?:EP|E|Episode)[ ._-]?(\d{1,4})(?:v\d)?(?![A-Za-z0-9])/i)))
    return +m[1]
  return null
}

function numTokens(name: string): number[] {
  const s = cleanFilename(name)
  const out: number[] = []
  const re = /(^|[^A-Za-z0-9])(\d{1,4})(?![A-Za-z0-9])/g
  let m: RegExpExecArray | null
  while ((m = re.exec(s))) {
    const v = +m[2]
    if (m[2].length === 4 && v >= 1950 && v <= 2039) continue
    out.push(v)
  }
  return out
}

function pickPosition(lists: number[][]) {
  const n = lists.length
  const maxLen = Math.max(0, ...lists.map((t) => t.length))
  let best: { mode: 'start' | 'end'; p: number; score: number; ratio: number } | null = null
  for (const mode of ['start', 'end'] as const) {
    for (let p = 0; p < maxLen; p++) {
      const vals: number[] = []
      for (const t of lists) {
        const i = mode === 'start' ? p : t.length - 1 - p
        if (i >= 0 && i < t.length) vals.push(t[i])
      }
      if (vals.length < Math.max(2, n * 0.5)) continue
      const set = new Set(vals)
      if (set.size < 2) continue
      const ratio = set.size / vals.length
      const span = Math.max(...set) - Math.min(...set) + 1
      const score = ratio * 1000 + set.size / span
      if (!best || score > best.score) best = { mode, p, score, ratio }
    }
  }
  return best && best.ratio >= 0.6 ? best : null
}

function episodeKeys(names: string[]): (number | null)[] {
  const ex = names.map(explicitEp)
  if (ex.filter((x) => x !== null).length >= Math.max(2, names.length * 0.5)) return ex
  const lists = names.map(numTokens)
  const best = pickPosition(lists)
  if (!best) return names.map(() => null)
  return lists.map((t) => {
    const i = best.mode === 'start' ? best.p : t.length - 1 - best.p
    return i >= 0 && i < t.length ? t[i] : null
  })
}

function similarity(a: string, b: string): number {
  let p = 0
  while (p < a.length && p < b.length && a[p] === b[p]) p++
  let s = 0
  while (s < a.length - p && s < b.length - p && a[a.length - 1 - s] === b[b.length - 1 - s]) s++
  return p + s
}

export function formatEpisodeLabel(k: number): string {
  if (k >= 10000) {
    return 'S' + String(Math.floor(k / 10000)).padStart(2, '0') + 'E' + String(k % 10000).padStart(2, '0')
  }
  return '第 ' + k + ' 集'
}

/**
 * 智能查找相邻集（上一集 / 下一集）
 */
export function findNeighborEpisode(
  allNames: string[],
  currentName: string,
  dir: -1 | 1
): { name?: string; key?: number; error?: string } {
  const vids = allNames.filter(isVideoFile)
  if (!vids.includes(currentName)) return { error: '当前选中的不是视频文件' }
  const keys = episodeKeys(vids)
  const entries = vids
    .map((name, i) => ({ name, key: keys[i] }))
    .filter((e): e is { name: string; key: number } => e.key !== null)

  const cur = entries.find((e) => e.name === currentName)
  const uniq = [...new Set(entries.map((e) => e.key))].sort((a, b) => a - b)
  if (!cur || uniq.length < 2) return { error: '未识别到集数序列（可能是单部电影）' }

  const idx = uniq.indexOf(cur.key) + dir
  if (idx < 0) return { error: '已经是第一集了' }
  if (idx >= uniq.length) return { error: '已经是最后一集了' }

  const candidates = entries.filter((e) => e.key === uniq[idx])
  candidates.sort((a, b) => similarity(b.name, currentName) - similarity(a.name, currentName))
  return { name: candidates[0].name, key: candidates[0].key }
}

export interface EpisodeInfo {
  isEpisode: boolean
  label?: string
  key?: number
  hasPrev: boolean
  hasNext: boolean
  prevName?: string
  nextName?: string
  totalEpisodes?: number
}

/**
 * 分析当前文件是否为剧集序列中的一集，并获取前后集信息
 */
export function getEpisodeInfo(allNames: string[], currentName: string): EpisodeInfo {
  const vids = allNames.filter(isVideoFile)
  if (!vids.includes(currentName)) {
    return { isEpisode: false, hasPrev: false, hasNext: false }
  }
  const keys = episodeKeys(vids)
  const entries = vids
    .map((name, i) => ({ name, key: keys[i] }))
    .filter((e): e is { name: string; key: number } => e.key !== null)

  const cur = entries.find((e) => e.name === currentName)
  const uniq = [...new Set(entries.map((e) => e.key))].sort((a, b) => a - b)
  if (!cur || uniq.length < 2) {
    return { isEpisode: false, hasPrev: false, hasNext: false }
  }

  const curIdx = uniq.indexOf(cur.key)
  const hasPrev = curIdx > 0
  const hasNext = curIdx >= 0 && curIdx < uniq.length - 1

  let prevName: string | undefined
  if (hasPrev) {
    const candidates = entries.filter((e) => e.key === uniq[curIdx - 1])
    candidates.sort((a, b) => similarity(b.name, currentName) - similarity(a.name, currentName))
    prevName = candidates[0]?.name
  }

  let nextName: string | undefined
  if (hasNext) {
    const candidates = entries.filter((e) => e.key === uniq[curIdx + 1])
    candidates.sort((a, b) => similarity(b.name, currentName) - similarity(a.name, currentName))
    nextName = candidates[0]?.name
  }

  return {
    isEpisode: true,
    label: cur.key !== undefined ? formatEpisodeLabel(cur.key) : undefined,
    key: cur.key,
    hasPrev,
    hasNext,
    prevName,
    nextName,
    totalEpisodes: uniq.length,
  }
}

/* ============================================================================
 * OpenList API 交互（测试连接、刷新缓存）
 * ============================================================================ */

/**
 * 测试 OpenList 服务的连通性及登录凭据
 * 优先通过后端代理进行测试（避免浏览器 CORS 拦截）
 */
export async function testOpenListConnection(
  baseUrl: string,
  user?: string,
  pass?: string
): Promise<{ success: boolean; message: string }> {
  try {
    return await testOpenListBackend({
      baseUrl,
      username: user,
      password: pass,
    })
  } catch (e: any) {
    return { success: false, message: e.message || '连接 OpenList 失败，请检查服务地址与网络' }
  }
}

/**
 * 刷新 OpenList 的网盘目录缓存
 * 由后端发起请求，无需在浏览器端暴露密码，且完全避免 CORS 跨域问题
 */
export async function refreshOpenListDir(
  account: AccountLike | null | undefined,
  dir: string,
  config: OpenListConfig
): Promise<number> {
  const uidNum = account?.uid ? Number(account.uid) : undefined
  const username = account?.username

  try {
    const res = await refreshOpenListBackend({
      dir,
      uid: isNaN(uidNum as number) ? undefined : uidNum,
      username,
    })
    return res.total
  } catch (e: any) {
    throw new Error(e.message || '刷新 OpenList 目录缓存失败')
  }
}
