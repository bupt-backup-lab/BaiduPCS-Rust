<template>
  <el-dialog
      v-model="visible"
      title="复制 / 移动到其他账号"
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
          </el-radio-group>
        </el-form-item>
        <el-form-item label="目标账号">
          <AccountSelect
              v-model="targetUid"
              :exclude-uids="sourceUid != null ? [sourceUid] : []"
              placeholder="选择目标账号"
              class="cross-account__account"
          />
        </el-form-item>
        <el-form-item label="目标目录">
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
          通过临时私密分享实现：源账号建分享 → 目标账号转存 → 立即撤销分享。
          目标目录不能有同名的文件/文件夹（百度转存不会自动改名），开始前会先检查。
          <template v-if="mode === 'move'">
            移动仅在全部转存成功、且逐项核对目标与源的文件数和大小一致后，才删除源文件（进入源账号回收站）。
          </template>
        </template>
      </el-alert>
    </div>

    <!-- 进度 / 结果 -->
    <div v-else class="cross-account">
      <div class="cross-account__route">
        {{ job.mode === 'copy' ? '复制' : '移动' }} {{ job.paths.length }} 项：
        {{ accountName(job.source_uid) }} → {{ accountName(job.target_uid) }}
        <code>{{ job.target_dir }}</code>
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
        {{ CROSS_ACCOUNT_STAGE_LABELS[job.stage] }}
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
          开始{{ mode === 'copy' ? '复制' : '移动' }}
        </el-button>
      </template>
      <template v-else-if="tracking">
        <el-button @click="visible = false">后台运行</el-button>
      </template>
      <template v-else>
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
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { ElMessage, ElNotification } from 'element-plus'
import { Loading } from '@element-plus/icons-vue'
import AccountSelect from '@/components/AccountSelect.vue'
import NetdiskFolderPickerModal from '@/components/NetdiskFolderPickerModal.vue'
import { useAuthStore } from '@/stores/auth'
import { useIsMobile } from '@/utils/responsive'
import type { FileItem } from '@/api/file'
import {
  CROSS_ACCOUNT_STAGE_LABELS,
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
/** 连续轮询失败多少次后放弃 */
const MAX_POLL_FAILURES = 10

const authStore = useAuthStore()
const isMobile = useIsMobile()

const visible = computed({
  get: () => props.modelValue,
  set: (v: boolean) => emit('update:modelValue', v),
})

const sourceUid = computed(() => authStore.activeUid)
const mode = ref<CrossAccountMode>('copy')
const targetUid = ref<number | null>(null)
const targetDir = ref('/')
const showFolderPicker = ref(false)
const submitting = ref(false)

const job = ref<CrossAccountJob | null>(null)
/** 是否仍在跟踪任务（放弃轮询后为 false，再次打开时回到表单） */
const tracking = ref(false)
const pollError = ref('')
let pollTimer: ReturnType<typeof setTimeout> | null = null
let pollFailures = 0

const fileNames = computed(() =>
    props.files.map((f) => f.original_name || f.server_filename).join('、'),
)
const fileSummary = computed(() => {
  const names = props.files.map((f) => f.original_name || f.server_filename)
  if (names.length <= 2) return names.join('、')
  return `${names.slice(0, 2).join('、')} 等 ${names.length} 项`
})

const canSubmit = computed(
    () => props.files.length > 0 && targetUid.value != null && sourceUid.value != null,
)

const steps = computed(() => {
  const base = ['检查并分享', '转存', '撤销分享']
  return job.value?.mode === 'move' ? [...base, '核对并删除源文件'] : base
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
  const action = j.mode === 'copy' ? '复制' : '移动'
  const suffix = j.mode === 'move' && !j.source_deleted ? '（源文件未删除）' : ''
  return `已${action}到 ${accountName(j.target_uid)} 的 ${j.target_dir}${suffix}`
})

function resetForm() {
  mode.value = props.initialMode
  const candidates = authStore.accounts.filter((a) => a.uid !== sourceUid.value)
  targetUid.value = candidates.length === 1 ? candidates[0].uid : null
  targetDir.value = '/'
  job.value = null
  pollError.value = ''
}

// 打开时：仍在跟踪的任务继续展示，否则按本次选中的文件重置表单
watch(
    () => props.modelValue,
    (open) => {
      if (open && !tracking.value) resetForm()
    },
)

// 换目标账号后目录回到根（旧路径属于另一个账号）
watch(targetUid, () => {
  targetDir.value = '/'
})

async function submit() {
  if (!canSubmit.value || targetUid.value == null || sourceUid.value == null) return
  submitting.value = true
  try {
    job.value = await createCrossAccountJob({
      source_uid: sourceUid.value,
      target_uid: targetUid.value,
      paths: props.files.map((f) => f.path),
      target_dir: targetDir.value,
      mode: mode.value,
    })
    pollFailures = 0
    tracking.value = true
    schedulePoll()
  } catch (e: unknown) {
    ElMessage.error(e instanceof Error ? e.message : '创建任务失败')
  } finally {
    submitting.value = false
  }
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
    const action = j.mode === 'copy' ? '复制' : '移动'
    if (j.stage === 'failed') {
      ElNotification.error({ title: `跨账号${action}失败`, message: j.error || '操作失败' })
    } else if (j.warnings.length > 0) {
      ElNotification.warning({ title: `跨账号${action}完成，但有问题`, message: j.warnings.join('；') })
    } else {
      ElNotification.success({ title: `跨账号${action}完成`, message: successTitle.value })
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
