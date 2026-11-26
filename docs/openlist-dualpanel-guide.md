# OpenList 联动与双栏管理功能说明文档（`dualpanel` 分支）

本文档汇总了 `dualpanel` 分支在原版 **BaiduPCS-Rust** 基础上新增与修改的所有文件、架构设计与功能细节，方便日常使用、排查问题以及后续同步上游代码时参考。

---

## 1. 功能总览

`dualpanel` 分支围绕 **BaiduPCS-Rust × OpenList** 深度联动打造，主要包含五大模块：

1. **OpenList 服务端配置与代理网关**：配置持久化于 `config/app.toml` 的 `[openlist]` 段，由 Rust 后端统一代理认证、目录刷新、文件系统操作和 TMDB 元数据请求，规避浏览器 CORS 限制并避免在前端暴露 OpenList 密码。
2. **文件操作接管模式（`off` / `observe` / `on`）**：支持将网盘内的 `mkdir`、`rename`、`delete`、`move`、`copy` 五类同盘操作交由 OpenList 执行，顶栏提供一键切换标签，失败时支持一键回退 BaiduPCS 原生接口。
3. **多端媒体直链播放与剧集连播**：支持桌面端自定义协议（如 `mpvpcs://`）、Android Intent（系统选择器或指定包名）、iOS URL Scheme（VLC / Infuse / nPlayer 等）以及浏览器直链打开；自动按 Jellyfin 规范匹配同目录外挂字幕，并提供悬浮剧集连播控制栏。
4. **OpenList 双栏文件管理面板**：仿照经典双栏文件管理器（PC 左右分栏、移动端 A/B Tab 切换），支持跨栏复制/移动、扩展名标签快选、正则/关键字/前 N 项筛选，并内置三种高级整理工具：
   - **批量重命名**（正则 / 顺序递增 / 查找替换）
   - **智能影视重命名**（剧集/季度识别、拖拽排序、行内铅笔编辑、字幕后缀保留与实时冲突校验）
   - **TMDB 智能刮削**（中英文检索、关联 IMDb ID、豆瓣/IMDb/TMDB 外链跳转、4 种标准命名格式一键重命名）
5. **构建与发布工作流调整**：移除 Docker 相关配置，新增前端独立构建工作流及 `windows-arm64` 编译目标。

---

## 2. 新增与修改文件清单

### 2.1 新增文件

| 类型 | 文件路径 | 说明 |
| :--- | :--- | :--- |
| 后端配置 | `backend/src/config/openlist.rs` | 定义 `OpenListConfig` 与 `OpenListAccountMapping` 数据结构及默认值（支持 `camelCase` / `snake_case` 反序列化兼容）。 |
| 后端接口 | `backend/src/server/handlers/openlist.rs` | 实现 `/config/openlist`、`/openlist/test`、`/openlist/refresh`、`/openlist/execute`、`/openlist/fs/call`、`/openlist/tmdb/proxy` 代理接口，内置连接池复用、401 自动重新登录与 `not found` 刷新重试。 |
| 前端类型 | `frontend/src/types/openlist.ts` | 定义 `OpenListConfig`、`OpenListAccountMapping`、`OpenListTakeoverMode`、`OpenListExecuteOpRequest` 等 TypeScript 类型。 |
| 前端状态 | `frontend/src/stores/openlist.ts` | Pinia store（`useOpenListStore`），负责加载/保存 `[openlist]` 配置（后端 `app.toml` 为主、`localStorage` 降级缓存）及切换接管模式。 |
| 前端工具 | `frontend/src/utils/openlist.ts` | 账号挂载点解析、OpenList 直链/目录链接生成、多端播放器唤起（`launchMpv`）、Jellyfin 同名字幕匹配（`findMatchingSubtitles`）、剧集智能识别与上下集推导（`findNeighborEpisode` / `getEpisodeInfo`）。 |
| 前端接管 | `frontend/src/utils/openlistTakeover.ts` | 统一拦截网盘原生写操作（`handleTakeoverOperation`），按 `off` / `observe` / `on` 模式分流并处理失败回退确认。 |
| 前端组件 | `frontend/src/components/EpisodePlayerBar.vue` | 右下角悬浮剧集连播控制栏（支持上一集、下一集、复制直链、重播、关闭）。 |
| 前端组件 | `frontend/src/components/OpenListDualPaneModal.vue` | OpenList 双栏文件管理弹窗，包含双栏浏览、筛选、批量重命名、智能影视重命名与 TMDB 智能刮削子面板。 |
| 前端设置 | `frontend/src/components/settings/OpenListSettingsSection.vue` | 系统设置页中的「OpenList 联动设置」表单（基础地址、专用账号、桌面/Android/iOS 播放器参数、接管模式、账号挂载点映射表）。 |
| 工作流 | `.github/workflows/build-frontend.yml` | 前端独立快速构建与产物打包工作流（`frontend-dist.zip`）。 |
| 项目文档 | `docs/openlist-dualpanel-guide.md` | 本文档，记录 `dualpanel` 分支新增文件与功能详情。 |

### 2.2 修改与移除文件

| 状态 | 文件路径 | 变更内容 |
| :--- | :--- | :--- |
| 修改 | `backend/src/config/mod.rs` | 注册 `pub mod openlist`，并在 `AppConfig` 中新增 `openlist: OpenListConfig` 字段。 |
| 修改 | `backend/src/lib.rs` | 导出 `OpenListAccountMapping`、`OpenListConfig`。 |
| 修改 | `backend/src/main.rs` | 注册 6 条 OpenList 配置与代理路由。 |
| 修改 | `backend/src/server/handlers/mod.rs` | 注册并导出 `handlers::openlist`。 |
| 修改 | `backend/src/server/handlers/config.rs` | 在全局 `PUT /api/v1/config` 更新时保留 `openlist` 配置段，防止旧请求体意外将其覆盖为默认值。 |
| 修改 | `config/app.toml.example` | 补充 `[openlist]` 完整配置示例与各平台播放器说明。 |
| 修改 | `frontend/src/api/config.ts` | 新增 `getOpenListConfig`、`updateOpenListConfig`、`testOpenListBackend`、`refreshOpenListBackend`、`executeOpenListOpBackend`、`callOpenListFsBackend`、`proxyTmdbBackend` API 封装。 |
| 修改 | `frontend/src/api/file.ts` | 将 `createFolder`、`deleteFiles`、`copyFiles`、`moveFiles`、`renameFile` 接入 `handleTakeoverOperation` 接管拦截层。 |
| 修改 | `frontend/src/layouts/MainLayout.vue` | 在顶栏右侧增加 `OL·关闭` / `OL·观察` / `OL·接管` 快捷状态胶囊，点击可循环切换模式。 |
| 修改 | `frontend/src/views/FilesView.vue` | 工具栏新增「跳转OL」「刷新OL」「双栏管理」按钮；右键菜单新增「MPV 播放」「浏览器打开」「复制 OpenList 链接」；挂载 `EpisodePlayerBar` 与 `OpenListDualPaneModal`；优化移动端工具栏与卡片间距。 |
| 修改 | `frontend/src/views/SettingsView.vue` | 引入 `OpenListSettingsSection` 并在左侧设置导航增加「OpenList 联动」锚点。 |
| 修改 | `.github/workflows/release-binaries.yml` | 改用纯二进制 + 前端静态资源打包，新增 `windows-arm64`（`aarch64-pc-windows-msvc`）目标，使用 `cargo-zigbuild` 编译 Linux musl 目标。 |
| 删除 | `Dockerfile`、`docker-compose*.yml`、`.github/workflows/docker-publish.yml`、`.github/workflows/release-docker-arm64.yml` | 移除分支不再使用的 Docker 构建与发布配置。 |

---

## 3. 核心功能详解

### 3.1 OpenList 配置与账号挂载映射

配置持久化保存在 `config/app.toml` 的 `[openlist]` 段，可通过系统设置页的「OpenList 联动设置」直接维护：

- **基础连接**：
  - `enabled`：总开关，关闭时所有前端入口（工具栏按钮、右键菜单、顶栏状态胶囊）均自动隐藏。
  - `baseUrl`（TOML 字段 `base_url` / 别名 `baseUrl`）：OpenList 服务访问根地址（不含末尾 `/` 和 `/d/`）。
  - `username` / `password`：具有目录刷新与文件写入权限的 OpenList 账号。接口 `GET /config/openlist` 会将密码脱敏为空字符串并返回 `hasPassword` 标记；保存时若未输入新密码，后端自动保留原密码。
- **账号挂载映射（`accounts`）**：
  - 支持按 **百度 UID** 或 **百度用户名** 映射 `{ mount, root, base }`，其中 `root` 默认为 `/`，`base` 可选用于指定该账号独立的 OpenList 地址。
  - 匹配优先级：`accounts[uid]` → `accounts[username]` → `autoMountByUsername` 开启时自动按 `/{username}` 推导 → `fallbackMount` 兜底路径。

### 3.2 后端代理 API 一览

所有 OpenList 通信均经由后端路由转发（位于 `backend/src/server/handlers/openlist.rs`），浏览器不直接接触 OpenList 密码：

| 方法与路径 | 用途 | 核心机制 |
| :--- | :--- | :--- |
| `GET /config/openlist` | 读取 OpenList 配置 | 密码字段脱敏，仅返回 `hasPassword: bool`。 |
| `PUT /config/openlist` | 更新并落盘 `[openlist]` 配置 | 写入 `config/app.toml` 并更新内存 `AppState`；空密码保持原值。 |
| `POST /openlist/test` | 测试连通性与凭据 | 有账号密码时调用 `/api/auth/login` 验证认证；否则调用 `/api/fs/list` 做访客连通性测试。 |
| `POST /openlist/refresh` | 强制刷新指定目录缓存 | 将网盘路径转换为 OpenList 挂载路径后调用 `/api/fs/list`（`refresh: true`）。 |
| `POST /openlist/execute` | 执行操作接管 | 支持 `mkdir` / `rename` / `delete` / `move` / `copy`；按父目录或 `(src_dir, dst_dir)` 自动分组批处理，内置 401 自动重新登录和 `not found` 刷缓存重试。 |
| `POST /openlist/fs/call` | 通用 `/api/fs/*` 代理 | 供双栏管理面板调用（如 `/api/fs/list`、`/api/fs/batch_rename` 等），校验路径仅允许 `/api/fs/` 或 `/api/public/` 前缀并拦截路径穿越。 |
| `POST /openlist/tmdb/proxy` | TMDB API 请求代理 | 仅允许代理 `https://api.themoviedb.org/` 请求，解决浏览器端跨域及网络环境访问问题。 |

### 3.3 文件操作接管模式（`off` / `observe` / `on`）

在 `frontend/src/api/file.ts` 中，网盘页面的新建文件夹、重命名、删除、同盘移动、同盘复制操作都会经过 `handleTakeoverOperation`：

- **`off`（关闭）**：完全走 BaiduPCS 原生接口。
- **`observe`（观察模式）**：操作前弹出提示说明该操作可由 OpenList 接管，但实际仍由 BaiduPCS 原生执行，适合验证挂载映射是否正确。
- **`on`（接管模式）**：直接通过 `/openlist/execute` 由 OpenList 执行，同时自动刷新目录缓存。
  - **自动回退规则**：若当前账号未配置挂载点，或在移动/复制的同时进行了重命名（OpenList 无法一步完成），会自动退回 BaiduPCS 原生接口。
  - **异常兜底**：若 OpenList 返回失败，前端会弹窗询问是否改用 BaiduPCS 原生方式重试该次操作。

顶栏右侧的 `OL·关闭` / `OL·观察` / `OL·接管` 胶囊支持点击一键循环切换并持久化保存到后端。

### 3.4 多端播放器唤起、外挂字幕与剧集连播

#### 多端播放策略（`launchMpv`）

- **桌面端（Windows / macOS / Linux）**：将 `http(s)://` 替换为自定义协议（默认 `mpvpcs://`），若匹配到同目录字幕，将字幕直链作为 `?sub=...` 查询参数拼接到链接末尾，供本地协议处理脚本转换为播放器 `--sub-file` 参数。
- **Android 端**：构造 `intent://...#Intent;scheme=...;action=android.intent.action.VIEW;type=video/*;...` 唤起本地播放器，并附带首个匹配字幕的 `S.subtitles_location` 参数：
  - `androidPackage` 留空：不指定 `package=`，由 Android 系统弹出「打开方式」应用列表供用户自由选择。
  - 填写包名（如 `org.videolan.vlc`、`is.xyz.mpv`、`com.mxtech.videoplayer.ad` 等）：直接静默直达对应播放器。
- **iOS 端**：根据 `iosScheme` 构造对应回调链接（支持 `vlc`、`infuse`、`nplayer-https`、`vidhub`、`fileball` 或自定义 Scheme），其中 `vlc` 与 `infuse` 支持自动附带 `&sub=` 外挂字幕参数；留空时使用浏览器新标签页播放。

#### 同目录外挂字幕自动匹配（`findMatchingSubtitles`）

支持 `.srt` / `.ass` / `.ssa` / `.vtt` / `.sub` / `.idx` / `.sup` 格式，按 Jellyfin 命名规范匹配完全同名或以 `视频基名.` 开头的字幕文件，并自动将中文标签（`zh` / `chi` / `zho` / `chs` / `cht` / `简` / `繁` / `中字` / `双语`）排在首位优先加载。

#### 剧集连播悬浮栏（`EpisodePlayerBar.vue`）

无论是主文件列表还是双栏管理面板中播放视频，当 `getEpisodeInfo` 在当前目录识别到不少于 2 集的剧集序列时，右下角会自动弹出悬浮连播栏：
- 支持显式命名（`S01E05`、`1x05`、`第5集`、`EP05`）与位置数字序列自动推理（自动过滤年份、分辨率、编码如 `1080p`/`x265`/`10bit`、CRC32 等干扰数字）。
- 提供 **上一集**、**下一集**、**复制当前集直链**、**重新唤起播放器** 四个快捷按钮。

### 3.5 双栏管理面板与影视工具套件（`OpenListDualPaneModal.vue`）

点击文件页工具栏的「双栏管理」按钮打开，初始自动定位到当前百度网盘目录对应的 OpenList 路径：

- **双栏浏览与跨栏操作**：
  - 左侧面板 A、右侧面板 B 各自独立导航、刷新、新建文件夹，支持「同步路径给对侧」。
  - 移动端自动切换为顶部 A / B Tab 视图，底部显示操作方向提示（`A → B` 或 `B → A`），上下文菜单自动适应为底部抽屉样式。
  - 支持按 **正则**、**关键字**、**选前 N 个** 筛选勾选，以及按文件扩展名标签（如 `.mkv (12/12)`）一键反选/全选，并配有双栏内嵌删除确认框。
- **批量重命名**：
  - 支持 **正则表达式替换**、**顺序重命名**（支持 `{number}` 占位符、起始数值与前导零宽度）、**查找并替换** 三种模式，修改前先在表格预览新旧文件名对比。
- **智能影视重命名**：
  - 自动识别父目录是否为季度文件夹（`S01` / `Season 1`），自动切换为「剧集模式」或「季度模式」。
  - 支持 **智能正则提取**（识别 `SxxExx`、多集连号 `E01-E02` 等）与 **强制顺序递增**（可设起始号与补零位）。
  - 剧集前缀支持「仅剧名」「纯净 `SxxExx`」「附加目录名」「自定义前缀」四档切换；字幕文件支持智能保留原语言标签、快捷预设（`.简中.zh` 等）或自定义后缀。
  - 支持 **列表反转**、**拖拽排序**、**单行铅笔原地微调**，实时检测名称重复（🚨）、与原序号不匹配（❌）和跳号（⚠️），并将前缀/字幕偏好记忆在 `localStorage` 中。
- **TMDB 智能刮削**：
  - 右键菜单选择「TMDB 智能刮削」，自动清理文件名中的画质/编码/季度标签提取影视名和年份。
  - 前端直连 TMDB 失败时自动无缝降级走后端 `/openlist/tmdb/proxy` 代理。
  - 并发查询外链 IMDb ID，提供 **TMDB**、**豆瓣**、**IMDb** 一键外链查证，并支持 4 种规范格式（`剧名 (年份) [tmdbid-xxx]` 等）一键应用重命名（记忆上次选用的格式）。

---

## 4. 后续同步上游更新建议

`dualpanel` 分支采用基于上游 `main` 的 `rebase` 策略维护。后续当你的 GitHub `main` 再次同步上游更新后，只需执行：

```bash
git fetch origin
git checkout dualpanel
git branch backup/dualpanel-$(date +%Y%m%d)   # 可选：操作前留一个备份分支
git rebase origin/main
git push --force-with-lease origin dualpanel
```
