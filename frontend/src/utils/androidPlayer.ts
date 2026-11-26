import { reactive } from 'vue'
import { ElMessage } from 'element-plus'

export interface AndroidPlayerPreset {
  pkg: string
  name: string
  desc: string
}

export const ANDROID_PLAYER_PRESETS: AndroidPlayerPreset[] = [
  { pkg: 'org.videolan.vlc', name: 'VLC for Android', desc: '推荐：硬件解码稳定，支持外挂字幕' },
  { pkg: 'is.xyz.mpv', name: 'MPV for Android', desc: '开源高画质，支持 ASS 特效字幕' },
  { pkg: 'com.mxtech.videoplayer.ad', name: 'MX Player 免费版', desc: '经典安卓本地播放器' },
  { pkg: 'com.mxtech.videoplayer.pro', name: 'MX Player 专业版', desc: '无广告专业版' },
  { pkg: 'com.brouken.player', name: 'Just Player', desc: '轻量级 ExoPlayer 内核' },
  { pkg: 'org.courville.nova', name: 'Nova Video Player', desc: '支持海报墙与外挂字幕' },
  { pkg: 'org.xbmc.kodi', name: 'Kodi', desc: '全能媒体中心播放器' },
]

const CUSTOM_PKG_STORAGE_KEY = 'openlist:android_custom_pkg'

function loadLastCustomPackage(): string {
  try {
    return localStorage.getItem(CUSTOM_PKG_STORAGE_KEY) || ''
  } catch {
    return ''
  }
}

export function saveLastCustomPackage(pkg: string): void {
  try {
    const trimmed = pkg.trim()
    if (trimmed) {
      localStorage.setItem(CUSTOM_PKG_STORAGE_KEY, trimmed)
    }
  } catch {
    // 忽略隐私模式下 localStorage 不可用的情况
  }
}

export interface AndroidChooserState {
  visible: boolean
  url: string
  size?: number
  sizeText: string
  title?: string
  subUrls?: string[]
  lastCustomPackage: string
}

export const androidChooserState = reactive<AndroidChooserState>({
  visible: false,
  url: '',
  size: undefined,
  sizeText: '',
  title: undefined,
  subUrls: undefined,
  lastCustomPackage: '',
})

export function buildAndroidIntentUrl(url: string, pkg: string, subUrls?: string[]): string {
  // 附加 S.subtitles_location 参数以便 VLC 等主流播放器自动加载字幕
  let extra = ''
  if (subUrls && subUrls.length > 0) {
    extra += `S.subtitles_location=${encodeURIComponent(subUrls[0])};`
  }
  const trimmedPkg = pkg.trim()
  const pkgPart = trimmedPkg ? `package=${encodeURIComponent(trimmedPkg)};` : ''
  const urlScheme = /^https:\/\//i.test(url) ? 'https' : 'http'
  return (
    url.replace(/^https?:\/\//i, 'intent://') +
    `#Intent;scheme=${urlScheme};action=android.intent.action.VIEW;type=video/*;${pkgPart}${extra}end;`
  )
}

export function launchAndroidWithPackage(
  url: string,
  pkg: string,
  options?: { sizeText?: string; title?: string; subUrls?: string[] }
): void {
  const trimmedPkg = pkg.trim()
  if (!trimmedPkg) return

  const intent = buildAndroidIntentUrl(url, trimmedPkg, options?.subUrls)
  const preset = ANDROID_PLAYER_PRESETS.find((item) => item.pkg === trimmedPkg)
  const appLabel = preset ? preset.name : trimmedPkg
  const head = options?.title ? `${options.title}\n` : ''
  const sz = options?.sizeText || ''
  const subTip =
    options?.subUrls && options.subUrls.length > 0
      ? ` [已自动关联 ${options.subUrls.length} 个同名字幕]`
      : ''

  ElMessage.info(`${head}正在唤起 ${appLabel}${sz}${subTip}...`)
  // 必须在用户点击的同步调用栈中修改 location，避免移动浏览器拦截外部应用唤起
  window.location.href = intent
}

export function openAndroidPlayerChooser(params: {
  url: string
  size?: number
  sizeText?: string
  title?: string
  subUrls?: string[]
}): void {
  androidChooserState.url = params.url
  androidChooserState.size = params.size
  androidChooserState.sizeText = params.sizeText || ''
  androidChooserState.title = params.title
  androidChooserState.subUrls = params.subUrls
  androidChooserState.lastCustomPackage = loadLastCustomPackage()
  androidChooserState.visible = true
}

export async function copyTextWithFallback(text: string): Promise<boolean> {
  if (!text) return false
  if (navigator.clipboard && typeof navigator.clipboard.writeText === 'function') {
    try {
      await navigator.clipboard.writeText(text)
      return true
    } catch {
      // 非 HTTPS / 权限受限时退回 execCommand 方案
    }
  }
  try {
    const textarea = document.createElement('textarea')
    textarea.value = text
    textarea.setAttribute('readonly', '')
    textarea.style.position = 'fixed'
    textarea.style.top = '-9999px'
    textarea.style.left = '-9999px'
    document.body.appendChild(textarea)
    textarea.select()
    const ok = document.execCommand('copy')
    document.body.removeChild(textarea)
    return ok
  } catch {
    return false
  }
}
