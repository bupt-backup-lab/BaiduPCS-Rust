/**
 * OpenList 联动配置与类型定义
 */

export interface OpenListAccountMapping {
  /** OpenList 挂载路径，如 "/HKU慕容" 或 "/juna" */
  mount: string
  /** 对应的网盘根目录，默认 "/" */
  root?: string
  /** 可选：该账号独立使用的 OpenList 地址 */
  base?: string
}

/** 操作接管模式: "off" (关闭), "observe" (观察), "on" (接管) */
export type OpenListTakeoverMode = 'off' | 'observe' | 'on'

export interface OpenListConfig {
  /** 是否启用 OpenList 联动 */
  enabled: boolean
  /** OpenList 基础地址，如 "https://openlist.example.com:11443" */
  baseUrl: string
  /** OpenList 具有刷新/写入权限的专用用户名 */
  username: string
  /** OpenList 密码（输入新密码，已保存时为空） */
  password?: string
  /** 后端是否已保存密码 */
  hasPassword?: boolean
  /** 桌面端自定义协议名，用于唤起 mpv，默认 "mpvpcs" */
  desktopScheme: string
  /** 安卓端播放器包名：留空表示弹出选择器，支持指定如 "org.videolan.vlc", "is.xyz.mpv" 等 */
  androidPackage?: string
  /** iOS 端播放器 Scheme：留空表示浏览器播放，支持指定如 "vlc", "infuse", "nplayer-https" 等 */
  iosScheme?: string
  /** 未手动配置挂载路径时，是否自动按 "/用户名" 猜测 */
  autoMountByUsername: boolean
  /** 无法识别账号时的兜底挂载路径 */
  fallbackMount: string
  /** 操作接管模式: "off" (关闭), "observe" (观察), "on" (接管) */
  takeoverMode: OpenListTakeoverMode
  /** 开启接管模式时，跨账号移动最后一步「删除源文件」是否也交由 OpenList 执行 */
  crossAccountDeleteTakeover?: boolean
  /** 账号挂载路径映射表：百度用户名或 UID -> 挂载配置 */
  accounts: Record<string, OpenListAccountMapping>
}

export interface OpenListSpaceInfo {
  supported: boolean
  driver: string
  mountPath: string
  totalSpace: number | null
  usedSpace: number | null
  freeSpace: number | null
}

export interface OpenListFileOpItem {
  path: string
  dest: string
  newname?: string
}

export interface OpenListExecuteOpRequest {
  op: 'rename' | 'delete' | 'move' | 'copy' | 'mkdir'
  uid?: number
  username?: string
  path?: string
  new_name?: string
  paths?: string[]
  items?: OpenListFileOpItem[]
}

export interface OpenListExecuteOpResponse<T = any> {
  success: boolean
  desc: string
  data: T
}
