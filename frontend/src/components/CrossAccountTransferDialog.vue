<template>
  <el-dialog
      v-model="visible"
      :title="dialogTitle"
      :width="isMobile ? '92%' : '560px'"
      :close-on-click-modal="false"
      append-to-body
  >
    <!-- 表单 -->
    <div v-if="!job" class="cross-account">
      <div class="cross-account__files">
        <span class="label">已选</span>
        <span class="value" :title="fileNames">{{ fileSummary }}</span>
      </div>

      <el-form label-width="84px" label-position="left" @submit.prevent>
        <el-form-item label="操作">
          <el-radio-group v-model="mode">
            <el-radio-button value="copy">复制</el-radio-button>
            <el-radio-button value="move">移动</el-radio-button>
            <el-radio-button value="download">下载</el-radio-button>
          </el-radio-group>
        </el-form-item>
        <el-form-item :label="mode === 'download' ? '下载账号' : '目标账号'">
          <AccountSelect
              v-model="targetUid"
              :exclude-uids="sourceUid != null ? [sourceUid] : []"
              :placeholder="mode === 'download' ? '选择用来下载的账号' : '选择目标账号'"
              class="cross-account__account"
              @change="targetPicked = true"
          />
          <el-checkbox
              v-if="mode === 'download'"
              :model-value="isDefaultAccount"
              :disabled="targetUid == null || savingDefaultAccount"
              class="cross-account__default"
              @change="toggleDefaultAccount"
          >
            设为默认下载账号
          </el-checkbox>
        </el-form-item>
        <el-form-item v-if="mode === 'download'" label="下载到">
          <div class="cross-account__dir">
            <code class="cross-account__dir-value" :title="localDownloadPath">{{ localDownloadPath || '未选择' }}</code>
            <el-button size="small" @click="showDownloadPicker = true">选择</el-button>
          </div>
        </el-form-item>
        <el-form-item v-else label="目标目录">
          <div class="cross-account__dir">
            <code class="cross-account__dir-value">{{ targetDir }}</code>
            <el-button size="small" :disabled="targetUid == null" @click="showFolderPicker = true">
              选择
            </el-button>
          </div>
        </el-form-item>
      </el-form>

      <el-alert type="info" :closable="false" show-icon class="cross-account__tip">
        <template #title>
          <template v-if="mode === 'download'">
            相当于自动生成链接的「分享直下」：源账号建临时私密分享 → 下载账号转存到其网盘临时目录并自动下载 →
            转存完成即撤销分享。下载账号需有足够的网盘空间暂存这些文件；下载进度在「下载管理」查看，
            临时文件按分享直下设置清理。
          </template>
          <template v-else>
            通过临时私密分享实现：源账号建分享 → 目标账号转存 → 立即撤销分享。
            目标目录不能有同名的文件/文件夹（百度转存不会自动改名），开始前会先检查。
            <template v-if="mode === 'move'">
              移动仅在全部转存成功、且逐项核对目标与源的文件数和大小一致后，才删除源文件（进入源账号回收站）。
            </template>
          </template>
        </template>
      </el-alert>
    </div>

    <!-- 进度 / 结果 -->
    <div v-else class="cross-account">
      <div class="cross-account__route">
        {{ MODE_LABELS[job.mode] }} {{ job.paths.length }} 项：
        <template v-if="job.mode === 'download'">
          {{ accountName(job.source_uid) }} → 由 {{ accountName(job.target_uid) }} 下载到
          <code>{{ job.local_download_path }}</code>
        </template>
        <template v-else>
          {{ accountName(job.source_uid) }} → {{ accountName(job.target_uid) }}
          <code>{{ job.target_dir }}</code>
        </template>
      </div>

      <el-steps
          :active="activeStep"
          :process-status="job.stage === 'failed' ? 'error' : 'process'"
          finish-status="success"
          align-center
          class="cross-account__steps"
      >
        <el-step v-for="step in steps" :key="step" :title="step" />
      </el-steps>

      <div v-if="tracking" class="cross-account__stage">
        <el-icon class="is-loading"><Loading /></el-icon>
        {{ crossAccountStageLabel(job.mode, job.stage) }}
        <!-- transferred_count 只在转存结束时回填，过程中显示总数 -->
        <span v-if="job.stage === 'transferring' && job.transferred_count > 0">
          （{{ job.transferred_count }} / {{ job.total_count }}）
        </span>
        <span v-else-if="job.stage === 'transferring' && job.total_count > 0">
          （共 {{ job.total_count }} 项）
        </span>
      </div>

      <el-alert
          v-if="job.stage === 'completed'"
          type="success"
          :closable="false"
          show-icon
          :title="successTitle"
      />
      <el-alert
          v-if="job.stage === 'failed'"
          type="error"
          :closable="false"
          show-icon
          :title="job.error || '操作失败'"
      />
      <el-alert
          v-for="(w, i) in job.warnings"
          :key="i"
          type="warning"
          :closable="false"
          show-icon
          :title="w"
          class="cross-account__warning"
      />
      <div v-if="pollError" class="cross-account__poll-error">{{ pollError }}</div>
    </div>

    <template #footer>
      <template v-if="!job">
        <el-button @click="visible = false">取消</el-button>
        <el-button type="primary" :loading="submitting" :disabled="!canSubmit" @click="submit">
          开始{{ MODE_LABELS[mode] }}
        </el-button>
      </template>
      <template v-else-if="tracking">
        <el-button @click="visible = false">后台运行</el-button>
      </template>
      <template v-else>
        <el-button v-if="job.mode === 'download' && job.stage === 'completed'" @click="goToDownloads">
          查看下载
        </el-button>
        <el-button type="primary" @click="visible = false">完成</el-button>
      </template>
    </template>

    <NetdiskFolderPickerModal
        v-model="showFolderPicker"
        title="选择目标账号中的文件夹"
        :initial-path="targetDir"
        :uid="targetUid"
        @confirm="(p: string) => (targetDir = p)"
    />
    <FilePickerModal
        v-model="showDownloadPicker"
        mode="download"
        select-type="directory"
        title="选择下载目录"
        :initial-path="downloadConfig?.recent_directory || defaultDownloadDir"
        :default-download-dir="defaultDownloadDir"
        @confirm-download="handleConfirmDownload"
        @use-default="handleUseDefaultDownload"
    />
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage, ElNotification } from 'element-plus'
import { Loading } from '@element-plus/icons-vue'
import AccountSelect from '@/components/AccountSelect.vue'
import NetdiskFolderPickerModal from '@/components/NetdiskFolderPickerModal.vue'
import { FilePickerModal } from '@/components/FilePicker'
import { useAuthStore } from '@/stores/auth'
import { useIsMobile } from '@/utils/responsive'
import type { FileItem } from '@/api/file'
import {
  getConfig,
  setDefaultDownloadAccount,
  setDefaultDownloadDir,
  updateRecentDirDebounced,
  type DownloadConfig,
} from '@/api/config'
import {
  crossAccountStageLabel,
  createCrossAccountJob,
  getCrossAccountJob,
  isCrossAccountJobFinished,
  type CrossAccountJob,
  type CrossAccountMode,
} from '@/api/crossAccount'

const props = withDefaults(
    defineProps<{
      modelValue: boolean
      files: FileItem[]
      initialMode?: CrossAccountMode
    }>(),
    { initialMode: 'copy' },
)

const emit = defineEmits<{
  (e: 'update:modelValue', v: boolean): void
  /** 任务结束（成功或失败）；移动成功时父组件应刷新列表 */
  (e: 'finished', job: CrossAccountJob): void
}>()

const POLL_INTERVAL_MS = 1000
/** 百度 vip_type：2 = 超级会员 */
const SVIP_TYPE = 2
/** 连续轮询失败多少次后放弃 */
const MAX_POLL_FAILURES = 10

const MODE_LABELS: Record<CrossAccountMode, string> = {
  copy: '复制',
  move: '移动',
  download: '下载',
}

const authStore = useAuthStore()
const isMobile = useIsMobile()
const router = useRouter()

const visible = computed({
  get: () => props.modelValue,
  set: (v: boolean) => emit('update:modelValue', v),
})

const sourceUid = computed(() => authStore.activeUid)
const mode = ref<CrossAccountMode>('copy')
const targetUid = ref<number | null>(null)
/** 用户本次是否手动选过账号；选过就不再自动预选 */
const targetPicked = ref(false)
const savingDefaultAccount = ref(false)
/** 已设的默认下载账号；单独存放，下载配置加载失败时勾选也能生效 */
const defaultAccountUid = ref<number | null>(null)
/** 默认账号保存成功的次数；配置请求途中保存过，就不用请求结果覆盖刚存的值 */
let defaultAccountSaves = 0
const targetDir = ref('/')
const showFolderPicker = ref(false)
const submitting = ref(false)

// 下载模式：本地下载目录
const localDownloadPath = ref('')
const downloadConfig = ref<DownloadConfig | null>(null)
const showDownloadPicker = ref(false)
const defaultDownloadDir = computed(
    () => downloadConfig.value?.default_directory || downloadConfig.value?.download_dir || '',
)

const job = ref<CrossAccountJob | null>(null)
/** 是否仍在跟踪任务（放弃轮询后为 false，再次打开时回到表单） */
const tracking = ref(false)
const pollError = ref('')
let pollTimer: ReturnType<typeof setTimeout> | null = null
let pollFailures = 0

const dialogTitle = computed(() => {
  const m = job.value?.mode ?? mode.value
  return m === 'download' ? '用其他账号下载' : '复制 / 移动到其他账号'
})

const fileNames = computed(() =>
    props.files.map((f) => f.original_name || f.server_filename).join('、'),
)
const fileSummary = computed(() => {
  const names = props.files.map((f) => f.original_name || f.server_filename)
  if (names.length <= 2) return names.join('、')
  return `${names.slice(0, 2).join('、')} 等 ${names.length} 项`
})

const canSubmit = computed(
    () =>
        props.files.length > 0 &&
        targetUid.value != null &&
        sourceUid.value != null &&
        (mode.value !== 'download' || localDownloadPath.value.trim() !== ''),
)

const steps = computed(() => {
  switch (job.value?.mode) {
    case 'move':
      return ['检查并分享', '转存', '撤销分享', '核对并删除源文件']
    case 'download':
      return ['检查并分享', '转存到临时目录', '开始下载并撤销分享']
    default:
      return ['检查并分享', '转存', '撤销分享']
  }
})

/** 当前步骤下标；完成时为步骤总数（全部打勾） */
const activeStep = computed(() => {
  const j = job.value
  if (!j) return 0
  // 失败时停在出错的那一步
  const stage = j.stage === 'failed' ? (j.failed_stage ?? 'checking') : j.stage
  switch (stage) {
    case 'checking':
    case 'sharing':
      return 0
    case 'transferring':
      return 1
    case 'revoking':
      return 2
    case 'verifying':
    case 'deleting_source':
      return 3
    case 'completed':
      return steps.value.length
  }
  return 0
})

function accountName(uid: number): string {
  const acc = authStore.accounts.find((a) => a.uid === uid)
  return acc ? acc.nickname || acc.username || `UID:${uid}` : `UID:${uid}`
}

const successTitle = computed(() => {
  const j = job.value
  if (!j) return ''
  if (j.mode === 'download') {
    return `已由 ${accountName(j.target_uid)} 开始下载到 ${j.local_download_path}，进度请在「下载管理」查看`
  }
  const suffix = j.mode === 'move' && !j.source_deleted ? '（源文件未删除）' : ''
  return `已${MODE_LABELS[j.mode]}到 ${accountName(j.target_uid)} 的 ${j.target_dir}${suffix}`
})

/**
 * 自动预选的账号：下载模式依次取设好的默认下载账号、第一个 SVIP 账号；
 * 都没有时（以及复制/移动模式）只剩一个候选账号才直接选它
 */
function autoTargetUid(m: CrossAccountMode): number | null {
  const candidates = authStore.accounts.filter((a) => a.uid !== sourceUid.value)
  if (m === 'download') {
    const preferred =
        candidates.find((a) => a.uid === defaultAccountUid.value) ??
        candidates.find((a) => a.vip_type === SVIP_TYPE)
    if (preferred) return preferred.uid
  }
  return candidates.length === 1 ? candidates[0].uid : null
}

function applyAutoTarget() {
  if (!targetPicked.value) targetUid.value = autoTargetUid(mode.value)
}

const isDefaultAccount = computed(
    () => targetUid.value != null && targetUid.value === defaultAccountUid.value,
)

async function toggleDefaultAccount(checked: string | number | boolean) {
  if (targetUid.value == null) return
  const uid = checked ? targetUid.value : null
  savingDefaultAccount.value = true
  try {
    await setDefaultDownloadAccount(uid)
    defaultAccountUid.value = uid
    defaultAccountSaves += 1
  } catch (e) {
    // 错误提示已由 config API 的拦截器弹出
    console.error('设置默认下载账号失败:', e)
  } finally {
    savingDefaultAccount.value = false
  }
}

function resetForm() {
  mode.value = props.initialMode
  targetPicked.value = false
  targetUid.value = autoTargetUid(mode.value)
  targetDir.value = '/'
  job.value = null
  pollError.value = ''
}

let configLoading: Promise<void> | null = null

async function loadDownloadConfig() {
  const savesBefore = defaultAccountSaves
  try {
    downloadConfig.value = (await getConfig()).download
    if (defaultAccountSaves === savesBefore) {
      defaultAccountUid.value = downloadConfig.value.default_download_account_uid ?? null
    }
  } catch (e) {
    console.error('加载下载配置失败:', e)
  } finally {
    configLoading = null
  }
}

/**
 * 进入下载模式时读取下载配置（上次加载失败则重试，并发调用共用一次请求），
 * 本地目录默认用默认下载目录，账号按默认下载账号预选
 */
async function ensureDownloadConfig() {
  if (!downloadConfig.value) {
    if (!configLoading) configLoading = loadDownloadConfig()
    await configLoading
  }
  if (!localDownloadPath.value) {
    localDownloadPath.value = defaultDownloadDir.value
  }
  if (mode.value === 'download') applyAutoTarget()
}

// 打开时：仍在跟踪的任务继续展示，否则按本次选中的文件重置表单
watch(
    () => props.modelValue,
    (open) => {
      if (!open || tracking.value) return
      resetForm()
      // mode 没变时 watch(mode) 不会触发，这里补一次（配置已加载时只做预选）
      if (mode.value === 'download') void ensureDownloadConfig()
    },
)

watch(mode, (m) => {
  applyAutoTarget()
  if (m === 'download') void ensureDownloadConfig()
})

// 换目标账号后目录回到根（旧路径属于另一个账号）
watch(targetUid, () => {
  targetDir.value = '/'
})

async function handleConfirmDownload(payload: { path: string; setAsDefault: boolean }) {
  showDownloadPicker.value = false
  localDownloadPath.value = payload.path
  if (payload.setAsDefault) {
    try {
      await setDefaultDownloadDir({ path: payload.path })
      if (downloadConfig.value) downloadConfig.value.default_directory = payload.path
    } catch (e) {
      console.error('设置默认下载目录失败:', e)
    }
  }
  updateRecentDirDebounced({ dir_type: 'download', path: payload.path })
  if (downloadConfig.value) downloadConfig.value.recent_directory = payload.path
}

function handleUseDefaultDownload() {
  showDownloadPicker.value = false
  localDownloadPath.value = defaultDownloadDir.value
}

async function submit() {
  if (!canSubmit.value || targetUid.value == null || sourceUid.value == null) return
  submitting.value = true
  try {
    const base = {
      source_uid: sourceUid.value,
      target_uid: targetUid.value,
      paths: props.files.map((f) => f.path),
      mode: mode.value,
    }
    job.value = await createCrossAccountJob(
        mode.value === 'download'
            ? { ...base, local_download_path: localDownloadPath.value.trim() }
            : { ...base, target_dir: targetDir.value },
    )
    pollFailures = 0
    tracking.value = true
    schedulePoll()
  } catch (e: unknown) {
    ElMessage.error(e instanceof Error ? e.message : '创建任务失败')
  } finally {
    submitting.value = false
  }
}

function goToDownloads() {
  visible.value = false
  void router.push('/downloads')
}

function stopPolling() {
  if (pollTimer) {
    clearTimeout(pollTimer)
    pollTimer = null
  }
}

function schedulePoll() {
  stopPolling()
  pollTimer = setTimeout(poll, POLL_INTERVAL_MS)
}

async function poll() {
  pollTimer = null
  const current = job.value
  if (!current) return
  try {
    const latest = await getCrossAccountJob(current.id)
    pollFailures = 0
    pollError.value = ''
    job.value = latest
    if (isCrossAccountJobFinished(latest)) {
      tracking.value = false
      onFinished(latest)
      return
    }
  } catch (e: unknown) {
    pollFailures += 1
    if (pollFailures >= MAX_POLL_FAILURES) {
      tracking.value = false
      pollError.value = `无法获取任务状态：${e instanceof Error ? e.message : '未知错误'}。任务仍在后台执行`
      return
    }
  }
  schedulePoll()
}

function onFinished(j: CrossAccountJob) {
  emit('finished', j)
  // 对话框已收起（后台运行）时用通知告知结果
  if (!visible.value) {
    const action = MODE_LABELS[j.mode]
    // 下载模式且已建出下载任务（成功，或开始下载后失败）：点击通知跳到「下载管理」
    const hasDownloads = j.mode === 'download' && (j.stage !== 'failed' || j.failed_stage === 'revoking')
    const onClick = hasDownloads ? () => void router.push('/downloads') : undefined
    const withHint = (msg: string) => (onClick ? `${msg}（点击查看下载）` : msg)
    if (j.stage === 'failed') {
      ElNotification.error({ title: `跨账号${action}失败`, message: withHint(j.error || '操作失败'), onClick })
    } else if (j.warnings.length > 0) {
      ElNotification.warning({ title: `跨账号${action}完成，但有问题`, message: withHint(j.warnings.join('；')), onClick })
    } else {
      ElNotification.success({
        title: `跨账号${action}${j.mode === 'download' ? '已开始' : '完成'}`,
        message: withHint(successTitle.value),
        onClick,
      })
    }
  }
}

onBeforeUnmount(stopPolling)
</script>

<style scoped lang="scss">
.cross-account {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.cross-account__files {
  display: flex;
  gap: 8px;
  font-size: 14px;

  .label {
    flex: 0 0 auto;
    color: var(--el-text-color-secondary);
  }

  .value {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
}

.cross-account__account {
  width: 100%;
}

.cross-account__default {
  margin-top: 4px;
}

.cross-account__dir {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  min-width: 0;
}

.cross-account__dir-value {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  padding: 2px 8px;
  background: var(--el-fill-color-light);
  border-radius: 4px;
}

.cross-account__route {
  font-size: 14px;
  word-break: break-all;

  code {
    padding: 0 4px;
    background: var(--el-fill-color-light);
    border-radius: 4px;
  }
}

.cross-account__steps {
  margin: 4px 0;
}

.cross-account__stage {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  color: var(--el-text-color-regular);
}

.cross-account__warning + .cross-account__warning {
  margin-top: -4px;
}

.cross-account__poll-error {
  color: var(--el-color-danger);
  font-size: 13px;
}
</style>
