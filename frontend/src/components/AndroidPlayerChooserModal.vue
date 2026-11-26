<template>
  <teleport to="body">
    <div
      v-if="androidChooserState.visible"
      class="android-chooser-overlay"
      @click.self="closeChooser"
    >
      <div
        class="android-chooser-card"
        role="dialog"
        aria-modal="true"
        aria-labelledby="android-chooser-title"
      >
        <div class="android-chooser-header">
          <div class="android-chooser-heading">
            <h3 id="android-chooser-title" class="android-chooser-title">选择安卓播放器</h3>
            <p class="android-chooser-tip">
              将直接向所选播放器发送 Intent，避免系统 IntentResolver 无法响应。
            </p>
          </div>
          <button
            type="button"
            class="android-chooser-close"
            aria-label="关闭"
            @click="closeChooser"
          >
            ✕
          </button>
        </div>

        <div v-if="androidChooserState.title || androidChooserState.sizeText || subtitleCount > 0" class="android-chooser-media">
          <div v-if="androidChooserState.title" class="android-chooser-media-name" :title="androidChooserState.title">
            {{ androidChooserState.title }}
          </div>
          <div class="android-chooser-media-meta">
            <span v-if="androidChooserState.sizeText">{{ androidChooserState.sizeText.replace(/^[（(]|[）)]$/g, '') }}</span>
            <span v-if="subtitleCount > 0" class="android-chooser-sub-badge">
              已关联 {{ subtitleCount }} 个同名字幕
            </span>
          </div>
        </div>

        <div class="android-chooser-list">
          <button
            v-for="preset in ANDROID_PLAYER_PRESETS"
            :key="preset.pkg"
            type="button"
            class="android-chooser-item"
            @click="handleSelectPackage(preset.pkg)"
          >
            <div class="android-chooser-item-main">
              <span class="android-chooser-item-name">{{ preset.name }}</span>
              <span class="android-chooser-item-desc">{{ preset.desc }}</span>
            </div>
            <span class="android-chooser-item-pkg">{{ preset.pkg }}</span>
          </button>
        </div>

        <div class="android-chooser-custom">
          <el-input
            v-model="customPackage"
            placeholder="或输入自定义包名，例如 org.videolan.vlc"
            clearable
            size="default"
            @keyup.enter="handleLaunchCustom"
          />
          <el-button type="primary" :disabled="!customPackage.trim()" @click="handleLaunchCustom">
            打开
          </el-button>
        </div>

        <div class="android-chooser-remember">
          <el-checkbox v-model="rememberAsDefault">
            设为默认播放器（保存到设置，以后不再每次询问）
          </el-checkbox>
        </div>

        <div class="android-chooser-footer">
          <el-button size="small" @click="handleOpenInBrowser">
            浏览器直接打开
          </el-button>
          <el-button size="small" @click="handleCopyUrl">
            复制视频直链
          </el-button>
          <el-button size="small" text @click="closeChooser">
            取消
          </el-button>
        </div>
      </div>
    </div>
  </teleport>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { useOpenListStore } from '@/stores/openlist'
import {
  ANDROID_PLAYER_PRESETS,
  androidChooserState,
  copyTextWithFallback,
  launchAndroidWithPackage,
  saveLastCustomPackage,
} from '@/utils/androidPlayer'
import { openInBrowser } from '@/utils/openlist'

const customPackage = ref('')
const rememberAsDefault = ref(false)

const subtitleCount = computed(() => androidChooserState.subUrls?.length ?? 0)

watch(
  () => androidChooserState.visible,
  (visible) => {
    if (visible) {
      // 默认不勾选，确保未指定默认播放器时每次打开前都询问
      rememberAsDefault.value = false
      customPackage.value = androidChooserState.lastCustomPackage || ''
    }
  }
)

function closeChooser() {
  androidChooserState.visible = false
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape' && androidChooserState.visible) {
    closeChooser()
  }
}

onMounted(() => {
  window.addEventListener('keydown', handleKeydown)
})

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeydown)
})

function handleSelectPackage(pkg: string) {
  const trimmedPkg = pkg.trim()
  if (!trimmedPkg) {
    ElMessage.warning('请输入有效的安卓播放器包名')
    return
  }

  const targetUrl = androidChooserState.url
  const sizeText = androidChooserState.sizeText
  const title = androidChooserState.title
  const subUrls = androidChooserState.subUrls ? [...androidChooserState.subUrls] : undefined
  const shouldSaveDefault = rememberAsDefault.value

  closeChooser()
  // 保持在点击事件的同步栈内触发，避免移动端浏览器拦截 intent:// 跳转
  launchAndroidWithPackage(targetUrl, trimmedPkg, { sizeText, title, subUrls })

  if (shouldSaveDefault) {
    // 延迟获取 store，避免 App 挂载于 Web 认证页时触发受保护的 /config/openlist 请求
    const openListStore = useOpenListStore()
    void openListStore
      .saveConfig({ androidPackage: trimmedPkg })
      .then(() => {
        ElMessage.success(`已将默认安卓播放器保存为 ${trimmedPkg}`)
      })
      .catch(() => {
        ElMessage.warning('保存默认播放器失败，可在设置页手动配置')
      })
  }
}

function handleLaunchCustom() {
  const trimmed = customPackage.value.trim()
  if (!trimmed) {
    ElMessage.warning('请输入安卓播放器包名')
    return
  }
  saveLastCustomPackage(trimmed)
  handleSelectPackage(trimmed)
}

function handleOpenInBrowser() {
  const targetUrl = androidChooserState.url
  const size = androidChooserState.size
  const title = androidChooserState.title
  closeChooser()
  openInBrowser(targetUrl, size, title)
}

async function handleCopyUrl() {
  const ok = await copyTextWithFallback(androidChooserState.url)
  if (ok) {
    ElMessage.success('已复制视频直链')
    closeChooser()
  } else {
    ElMessage.error('复制失败，请检查浏览器剪贴板权限')
  }
}
</script>

<style scoped>
.android-chooser-overlay {
  position: fixed;
  inset: 0;
  z-index: 100020;
  background: rgba(15, 23, 42, 0.56);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
  backdrop-filter: blur(2px);
}

.android-chooser-card {
  width: 100%;
  max-width: 460px;
  max-height: 88vh;
  overflow-y: auto;
  background: var(--el-bg-color, #ffffff);
  color: var(--el-text-color-primary, #1f2937);
  border-radius: 14px;
  border: 1px solid var(--el-border-color-lighter, #e5e7eb);
  box-shadow: 0 20px 48px rgba(0, 0, 0, 0.24);
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.android-chooser-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.android-chooser-heading {
  min-width: 0;
}

.android-chooser-title {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  line-height: 1.35;
}

.android-chooser-tip {
  margin: 4px 0 0;
  font-size: 12px;
  color: var(--el-text-color-secondary, #6b7280);
  line-height: 1.4;
}

.android-chooser-close {
  flex: 0 0 auto;
  width: 28px;
  height: 28px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--el-text-color-secondary, #6b7280);
  cursor: pointer;
  font-size: 15px;
  line-height: 1;
}

.android-chooser-close:hover {
  background: var(--el-fill-color-light, #f3f4f6);
  color: var(--el-text-color-primary, #1f2937);
}

.android-chooser-media {
  padding: 8px 10px;
  border-radius: 8px;
  background: var(--el-fill-color-light, #f5f7fa);
  border: 1px solid var(--el-border-color-lighter, #ebeef5);
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.android-chooser-media-name {
  font-size: 13px;
  font-weight: 500;
  word-break: break-all;
  line-height: 1.35;
}

.android-chooser-media-meta {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  font-size: 12px;
  color: var(--el-text-color-secondary, #6b7280);
}

.android-chooser-sub-badge {
  color: var(--el-color-success, #16a34a);
  font-weight: 500;
}

.android-chooser-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.android-chooser-item {
  width: 100%;
  text-align: left;
  padding: 10px 12px;
  border-radius: 10px;
  border: 1px solid var(--el-border-color-light, #e5e7eb);
  background: var(--el-bg-color-overlay, #ffffff);
  color: inherit;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  transition: border-color 0.15s ease, background-color 0.15s ease;
}

.android-chooser-item:hover,
.android-chooser-item:active {
  border-color: var(--el-color-primary, #409eff);
  background: var(--el-color-primary-light-9, #ecf5ff);
}

.android-chooser-item-main {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.android-chooser-item-name {
  font-size: 14px;
  font-weight: 600;
}

.android-chooser-item-desc {
  font-size: 12px;
  color: var(--el-text-color-secondary, #6b7280);
}

.android-chooser-item-pkg {
  flex: 0 0 auto;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 11px;
  color: var(--el-text-color-secondary, #6b7280);
}

.android-chooser-custom {
  display: flex;
  align-items: center;
  gap: 8px;
}

.android-chooser-remember {
  padding-top: 2px;
}

.android-chooser-footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  flex-wrap: wrap;
  gap: 8px;
  padding-top: 4px;
  border-top: 1px solid var(--el-border-color-lighter, #ebeef5);
}

@media (max-width: 600px) {
  .android-chooser-overlay {
    align-items: flex-end;
    padding: 0;
  }

  .android-chooser-card {
    max-width: 100%;
    max-height: 90vh;
    border-radius: 16px 16px 0 0;
    padding: 16px 14px calc(14px + env(safe-area-inset-bottom, 0px));
  }

  .android-chooser-item {
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
  }
}
</style>
