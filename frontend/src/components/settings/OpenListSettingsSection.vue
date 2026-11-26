<template>
  <el-card class="setting-card openlist-settings-card" shadow="hover">
    <template #header>
      <div class="card-header">
        <el-icon :size="20" color="#409eff">
          <Link />
        </el-icon>
        <span class="header-title">OpenList 联动设置</span>
        <el-tag
          :type="form.enabled ? 'success' : 'info'"
          size="small"
          style="margin-left: auto"
        >
          {{ form.enabled ? '已启用' : '未启用' }}
        </el-tag>
      </div>
    </template>

    <div class="openlist-content">
      <el-alert
        type="info"
        :closable="false"
        show-icon
        style="margin-bottom: 20px"
      >
        <template #title>
          实现 BaiduPCS-Rust 与 OpenList 的双向联动：右键直接复制 OpenList 直链、MPV 流媒体播放、连续剧识别播上下集、浏览器免下载预览，以及目录相互跳转与强制刷新缓存。
        </template>
      </el-alert>

      <el-form label-position="top">
        <el-form-item label="启用 OpenList 联动">
          <el-switch
            v-model="form.enabled"
            active-text="开启"
            inactive-text="关闭"
          />
        </el-form-item>

        <template v-if="form.enabled">
          <el-form-item label="OpenList 网址 (Base URL)" required>
            <el-input
              v-model="form.baseUrl"
              placeholder="例如 https://openlist.example.com:11443"
              clearable
            />
            <div class="form-tip">
              OpenList 服务器对外访问地址（不带末尾斜杠、不带 /d/ 和挂载路径）。
            </div>
          </el-form-item>

          <el-row :gutter="20">
            <el-col :xs="24" :sm="12">
              <el-form-item label="OpenList 刷新账号 (专用账号)">
                <el-input
                  v-model="form.username"
                  placeholder="具有写入权限的专用用户名"
                  clearable
                />
                <div class="form-tip">
                  用于在网盘界面触发「刷新 OL」时登录鉴权（OpenList 强制刷新目录需要“写入”权限）。
                </div>
              </el-form-item>
            </el-col>
            <el-col :xs="24" :sm="12">
              <el-form-item label="OpenList 账号密码">
                <el-input
                  v-model="form.password"
                  type="password"
                  show-password
                  :placeholder="form.hasPassword ? '已保存密码（输入新密码以修改）' : '密码'"
                  clearable
                />
                <div class="form-tip">
                  密码将安全保存在后端的 <code>config/app.toml</code> 中，建议在 OpenList 后台新建只分配“写入”权限的专用账号。
                </div>
              </el-form-item>
            </el-col>
          </el-row>

          <el-row :gutter="20">
            <el-col :xs="24" :sm="12">
              <el-form-item label="电脑端 MPV 自定义协议 Scheme">
                <el-input
                  v-model="form.desktopScheme"
                  placeholder="默认 mpvpcs"
                  clearable
                />
                <div class="form-tip">
                  桌面端唤起 mpv 的自定义协议名（需在系统注册表/系统协议管理器中关联 mpv.exe）。
                </div>
              </el-form-item>
            </el-col>
            <el-col :xs="24" :sm="12">
              <el-form-item label="自动根据百度用户名推导挂载路径">
                <el-switch
                  v-model="form.autoMountByUsername"
                  active-text="未配置时推导为 /{百度用户名}"
                  inactive-text="不推导"
                />
                <div class="form-tip">
                  若下方未单独配置某个账号的挂载路径，是否默认尝试以 <code>/用户名</code> 访问。
                </div>
              </el-form-item>
            </el-col>
          </el-row>

          <el-row :gutter="20">
            <el-col :xs="24" :sm="12">
              <el-form-item label="安卓端播放器包名 (Android Package)">
                <el-select
                  v-model="form.androidPackage"
                  filterable
                  allow-create
                  default-first-option
                  clearable
                  placeholder="留空：弹出系统播放器选择列表"
                  style="width: 100%"
                >
                  <el-option label="弹出系统应用选择列表（推荐）" value="" />
                  <el-option label="VLC for Android (org.videolan.vlc)" value="org.videolan.vlc" />
                  <el-option label="MPV for Android (is.xyz.mpv)" value="is.xyz.mpv" />
                  <el-option label="MX Player 免费版 (com.mxtech.videoplayer.ad)" value="com.mxtech.videoplayer.ad" />
                  <el-option label="MX Player 专业版 (com.mxtech.videoplayer.pro)" value="com.mxtech.videoplayer.pro" />
                  <el-option label="Just Player (com.brouken.player)" value="com.brouken.player" />
                  <el-option label="Nova Video Player (org.courville.nova)" value="org.courville.nova" />
                  <el-option label="Kodi (org.xbmc.kodi)" value="org.xbmc.kodi" />
                </el-select>
                <div class="form-tip">
                  留空时由 Android 弹出「打开方式」列表；填写包名（如 <code>org.videolan.vlc</code>）可直接静默唤起并支持外挂字幕。
                </div>
              </el-form-item>
            </el-col>
            <el-col :xs="24" :sm="12">
              <el-form-item label="iOS 端播放器 Scheme">
                <el-select
                  v-model="form.iosScheme"
                  filterable
                  allow-create
                  default-first-option
                  clearable
                  placeholder="留空：使用浏览器内置播放"
                  style="width: 100%"
                >
                  <el-option label="浏览器内置原生播放（默认）" value="" />
                  <el-option label="VLC for Mobile (vlc)" value="vlc" />
                  <el-option label="Infuse (infuse)" value="infuse" />
                  <el-option label="nPlayer (nplayer-https)" value="nplayer-https" />
                  <el-option label="VidHub (vidhub)" value="vidhub" />
                  <el-option label="Fileball (fileball)" value="fileball" />
                </el-select>
                <div class="form-tip">
                  留空时使用 Safari/浏览器直接播放；选择或输入 Scheme（如 <code>vlc</code> / <code>infuse</code>）可直接唤起 App。
                </div>
              </el-form-item>
            </el-col>
          </el-row>

          <el-form-item label="文件操作接管模式">
            <el-radio-group v-model="form.takeoverMode">
              <el-radio-button label="off">关闭</el-radio-button>
              <el-radio-button label="observe">观察模式</el-radio-button>
              <el-radio-button label="on">接管模式</el-radio-button>
            </el-radio-group>
            <div class="form-tip">
              同网盘内的新建文件夹、复制、移动、删除、重命名是否交由 OpenList 执行。<br />
              • <b>关闭</b>：所有操作均走 BaiduPCS 原生接口；<br />
              • <b>观察模式</b>：仅弹窗提醒该操作可由 OpenList 接管，仍走原生接口执行；<br />
              • <b>接管模式</b>：真正改由 OpenList 执行，极大提高同盘文件整理效率并即时同步索引。<br />
              （提示：也可在页面顶栏点击 <code>OL·接管</code> / <code>OL·观察</code> 小标签随时一键快速切换）
            </div>
          </el-form-item>

          <el-form-item label="跨账号移动时接管删除源文件">
            <el-switch
              v-model="form.crossAccountDeleteTakeover"
              active-text="开启接管"
              inactive-text="仅走原生"
            />
            <div class="form-tip">
              开启后，当上方处于「接管模式」时，跨账号移动完成校验后的最后一步「删除源文件」将优先交由 OpenList 执行（可降低原生删文件触发风控的概率；失败或未配置挂载点时自动回退 BaiduPCS 原生删除）。
            </div>
          </el-form-item>

          <!-- 账号挂载对应关系 -->
          <el-divider content-position="left">
            <el-icon><User /></el-icon> 百度账号 ↔ OpenList 存储挂载点对应表
          </el-divider>

          <div class="account-mapping-desc">
            请为每个百度网盘账号指定对应在 OpenList 后台配置的「挂载路径」（例如 <code>/HKU慕容</code> 或 <code>/juna</code>）：
          </div>

          <el-table :data="accountRows" border style="width: 100%; margin-bottom: 20px">
            <el-table-column label="百度账号" min-width="160">
              <template #default="{ row }">
                <div class="account-cell">
                  <strong>{{ row.username || row.nickname || '未知用户' }}</strong>
                  <span class="uid-sub">(UID: {{ row.uid }})</span>
                </div>
              </template>
            </el-table-column>

            <el-table-column label="OpenList 挂载路径 (必填)" min-width="200">
              <template #default="{ row }">
                <el-input
                  v-model="row.mount"
                  placeholder="如 /HKU慕容 或 /juna"
                  @change="updateRowMapping(row)"
                />
              </template>
            </el-table-column>

            <el-table-column label="对应网盘根目录 (可选)" min-width="150">
              <template #default="{ row }">
                <el-input
                  v-model="row.root"
                  placeholder="默认 /"
                  @change="updateRowMapping(row)"
                />
              </template>
            </el-table-column>

            <el-table-column label="快捷操作" width="130" align="center">
              <template #default="{ row }">
                <el-button
                  link
                  type="primary"
                  size="small"
                  @click="autoFillRow(row)"
                >
                  填为 /{{ row.username || '用户名' }}
                </el-button>
              </template>
            </el-table-column>
          </el-table>
        </template>

        <div class="button-row">
          <el-button
            v-if="form.enabled"
            :loading="testing"
            @click="handleTestConnection"
          >
            <el-icon><Connection /></el-icon>
            测试 OpenList 连接
          </el-button>
          <el-button type="primary" :loading="saving" @click="handleSave">
            <el-icon><Check /></el-icon>
            保存配置
          </el-button>
        </div>
      </el-form>
    </div>
  </el-card>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { ElMessage } from 'element-plus'
import { Link, Check, Connection, User } from '@element-plus/icons-vue'
import { useOpenListStore } from '@/stores/openlist'
import { useAuthStore } from '@/stores/auth'
import { testOpenListConnection } from '@/utils/openlist'
import type { OpenListConfig } from '@/types/openlist'

const openListStore = useOpenListStore()
const authStore = useAuthStore()

const form = ref<OpenListConfig>({
  enabled: false,
  baseUrl: '',
  username: '',
  password: '',
  hasPassword: false,
  desktopScheme: 'mpvpcs',
  androidPackage: '',
  iosScheme: '',
  autoMountByUsername: true,
  fallbackMount: '',
  takeoverMode: 'observe',
  crossAccountDeleteTakeover: true,
  accounts: {},
})

const testing = ref(false)
const saving = ref(false)

interface AccountRow {
  uid: number | string
  username?: string
  nickname?: string | null
  mount: string
  root: string
}

const accountRows = computed<AccountRow[]>(() => {
  const currentAccounts = authStore.accounts || []
  return currentAccounts.map((acc) => {
    const key = String(acc.uid)
    const username = acc.username || ''
    const mapping =
      form.value.accounts[key] ||
      (username ? form.value.accounts[username] : undefined)

    return {
      uid: acc.uid,
      username: acc.username,
      nickname: acc.nickname,
      mount: mapping?.mount || '',
      root: mapping?.root || '/',
    }
  })
})

function updateRowMapping(row: AccountRow) {
  const key = row.username || String(row.uid)
  if (row.mount.trim()) {
    form.value.accounts[key] = {
      mount: row.mount.trim().startsWith('/') ? row.mount.trim() : '/' + row.mount.trim(),
      root: row.root.trim() || '/',
    }
  } else {
    delete form.value.accounts[key]
  }
}

function autoFillRow(row: AccountRow) {
  const name = row.username || row.nickname || String(row.uid)
  row.mount = '/' + name
  row.root = '/'
  updateRowMapping(row)
}

async function handleTestConnection() {
  if (!form.value.baseUrl) {
    ElMessage.warning('请先填写 OpenList 基础网址')
    return
  }
  testing.value = true
  try {
    const res = await testOpenListConnection(
      form.value.baseUrl,
      form.value.username,
      form.value.password
    )
    if (res.success) {
      ElMessage.success(res.message)
    } else {
      ElMessage.error(res.message)
    }
  } finally {
    testing.value = false
  }
}

async function handleSave() {
  saving.value = true
  try {
    await openListStore.saveConfig({
      ...form.value,
      baseUrl: form.value.baseUrl.trim(),
      username: form.value.username.trim(),
      password: form.value.password,
      desktopScheme: form.value.desktopScheme.trim() || 'mpvpcs',
      androidPackage: (form.value.androidPackage || '').trim(),
      iosScheme: (form.value.iosScheme || '').trim(),
      crossAccountDeleteTakeover: form.value.crossAccountDeleteTakeover ?? true,
    })
    ElMessage.success('OpenList 联动配置已保存至 app.toml')
  } catch (e: any) {
    ElMessage.error(e.message || '保存失败')
  } finally {
    saving.value = false
  }
}

onMounted(async () => {
  await openListStore.loadConfig()
  form.value = {
    ...openListStore.config,
    accounts: { ...openListStore.config.accounts },
  }
})
</script>

<style scoped lang="scss">
.openlist-settings-card {
  margin-bottom: 24px;

  .card-header {
    display: flex;
    align-items: center;
    gap: 8px;

    .header-title {
      font-size: 16px;
      font-weight: 600;
    }
  }

  .openlist-content {
    padding: 8px 0;
  }

  .form-tip {
    font-size: 12px;
    color: #909399;
    margin-top: 4px;
    line-height: 1.4;

    code {
      background: #f4f4f5;
      padding: 2px 4px;
      border-radius: 3px;
      color: #e6a23c;
    }
  }

  .account-mapping-desc {
    font-size: 13px;
    color: #606266;
    margin-bottom: 12px;

    code {
      background: #f4f4f5;
      padding: 2px 4px;
      border-radius: 3px;
    }
  }

  .account-cell {
    display: flex;
    flex-direction: column;

    .uid-sub {
      font-size: 11px;
      color: #909399;
    }
  }

  .button-row {
    display: flex;
    justify-content: flex-end;
    gap: 12px;
    margin-top: 24px;
  }
}
</style>
