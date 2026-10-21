<template>
  <div class="cron-editor">
    <el-input
        :model-value="modelValue ?? ''"
        placeholder="分 时 日 月 星期，如 */10 11-13 * * 6"
        clearable
        class="cron-input"
        @update:model-value="emit('update:modelValue', $event)"
    />
    <div class="cron-presets">
      <span>常用：</span>
      <el-button
          v-for="p in CRON_PRESETS"
          :key="p.expr"
          size="small"
          link
          type="primary"
          @click="applyPreset(p.expr)"
      >
        {{ p.label }}
      </el-button>
    </div>
    <div class="cron-preview">
      <template v-if="preview">
        <div v-if="preview.description" class="cron-desc">{{ preview.description }}</div>
        <div v-if="preview.error" class="cron-error">{{ preview.error }}</div>
        <div v-if="preview.valid && preview.next_runs.length" class="cron-next">
          接下来：{{ preview.next_runs.join('，') }}
        </div>
        <div class="cron-tz">
          按服务器时区 {{ formatUtcOffset(preview.utc_offset_minutes) }} 计算<span
            v-if="preview.utc_offset_minutes !== browserUtcOffsetMinutes"
            class="cron-error"
          >，与当前浏览器时区 {{ formatUtcOffset(browserUtcOffsetMinutes) }} 不一致（Docker 部署可设置环境变量 TZ，如 Asia/Shanghai）</span>
        </div>
      </template>
      <div v-else class="cron-tz">
        格式「分 时 日 月 星期」，支持 * , - /；相邻两次检查至少间隔 10 分钟
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * 分享同步 cron 轮询表达式编辑器（issue #179）
 *
 * 输入框 + 常用模板 + 实时解释（后端按服务器时区计算描述与接下来几次触发时刻）。
 * 父组件保存前调用暴露的 `invalidReason(expr)` 拦截已知无效的表达式；
 * 预览尚未返回时放行，由后端校验兜底。
 */
import { onUnmounted, ref, watch } from 'vue'
import { previewCron, type CronPreview } from '@/api/shareSync'

const props = defineProps<{
  modelValue?: string | null
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
}>()

const CRON_PRESETS = [
  { label: '每周六 11-13 点每 10 分钟', expr: '*/10 11-13 * * 6' },
  { label: '每晚 19-23 点每 30 分钟', expr: '*/30 19-23 * * *' },
  { label: '每小时整点', expr: '0 * * * *' },
  { label: '每天 8 点和 20 点', expr: '0 8,20 * * *' },
]

const preview = ref<CronPreview | null>(null)
/** preview 对应的表达式（已 trim），用于判断预览结果是否仍对应当前输入 */
const previewExpr = ref('')
const browserUtcOffsetMinutes = -new Date().getTimezoneOffset()
let timer: ReturnType<typeof setTimeout> | null = null
let seq = 0

watch(
    () => props.modelValue,
    (expr) => {
      if (timer) {
        clearTimeout(timer)
        timer = null
      }
      const trimmed = (expr || '').trim()
      if (!trimmed) {
        seq++
        preview.value = null
        previewExpr.value = ''
        return
      }
      timer = setTimeout(() => refresh(trimmed), 300)
    },
    { immediate: true },
)

async function refresh(expr: string) {
  const current = ++seq
  try {
    const result = await previewCron(expr)
    if (current !== seq) return
    preview.value = result
    previewExpr.value = expr
  } catch (e) {
    if (current !== seq) return
    preview.value = null
    previewExpr.value = ''
    console.error('cron preview failed', e)
  }
}

// 值未变化时 watch 不会触发（如上次预览请求失败后再点同一模板），此时直接重新预览
function applyPreset(expr: string) {
  if ((props.modelValue || '').trim() === expr) {
    refresh(expr)
    return
  }
  emit('update:modelValue', expr)
}

function formatUtcOffset(minutes: number): string {
  const sign = minutes >= 0 ? '+' : '-'
  const abs = Math.abs(minutes)
  const h = String(Math.floor(abs / 60)).padStart(2, '0')
  const m = String(abs % 60).padStart(2, '0')
  return `UTC${sign}${h}:${m}`
}

/** 已知无效时返回原因；为空或预览未返回时返回 null（交给后端校验） */
function invalidReason(expr: string | null | undefined): string | null {
  const trimmed = (expr || '').trim()
  if (!trimmed) return '请填写 cron 表达式'
  if (preview.value && previewExpr.value === trimmed && !preview.value.valid) {
    return `cron 表达式无效：${preview.value.error || '请检查格式'}`
  }
  return null
}

defineExpose({ invalidReason })

onUnmounted(() => {
  if (timer) clearTimeout(timer)
})
</script>

<style scoped lang="scss">
.cron-editor { width: 100%; }
.cron-input {
  max-width: 360px;
  :deep(.el-input__inner) { font-family: monospace; }
}
.cron-presets {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px 8px;
  margin-top: 4px;
  font-size: 12px;
  color: #909399;
  .el-button { margin-left: 0; }
}
.cron-preview { margin-top: 4px; font-size: 12px; line-height: 1.6; }
.cron-desc { color: var(--el-color-primary); }
.cron-error { color: var(--el-color-danger); }
.cron-next { color: var(--el-text-color-regular); }
.cron-tz { color: #909399; }
</style>
