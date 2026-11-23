<template>
  <transition name="episode-bar-fade">
    <div v-if="visible && episodeInfo.isEpisode" class="episode-player-bar">
      <!-- 头部：剧集标签、模式及文件名与关闭按钮 -->
      <div class="bar-header">
        <div class="bar-title-wrap">
          <el-tag size="small" type="primary" effect="dark" class="ep-tag">
            {{ episodeInfo.label || '剧集播放' }}
          </el-tag>
          <el-tag size="small" type="info" class="mode-tag">
            {{ playMode === 'mpv' ? 'MPV' : '浏览器' }}
          </el-tag>
          <el-tooltip :content="filename" placement="top" :show-after="400">
            <span class="file-name">{{ filename }}</span>
          </el-tooltip>
        </div>
        <el-button
          :icon="Close"
          size="small"
          circle
          text
          class="close-btn"
          title="关闭连播控制栏"
          @click="$emit('close')"
        />
      </div>

      <!-- 控制区：全部4个按钮放在单行中（上一集、下一集、直链、重播） -->
      <div class="bar-controls">
        <el-tooltip
          :content="episodeInfo.hasPrev ? `上一集：${episodeInfo.prevName}` : '已经是第一集'"
          placement="top"
        >
          <el-button
            size="small"
            class="ctrl-btn"
            :disabled="!episodeInfo.hasPrev"
            :icon="DArrowLeft"
            @click="$emit('switch', -1)"
          >
            上一集
          </el-button>
        </el-tooltip>

        <el-tooltip
          :content="episodeInfo.hasNext ? `下一集：${episodeInfo.nextName}` : '已经是最后一集'"
          placement="top"
        >
          <el-button
            type="primary"
            size="small"
            class="ctrl-btn"
            :disabled="!episodeInfo.hasNext"
            @click="$emit('switch', 1)"
          >
            下一集
            <el-icon class="el-icon--right"><DArrowRight /></el-icon>
          </el-button>
        </el-tooltip>

        <el-tooltip content="复制当前播放文件的 OpenList 直链" placement="top">
          <el-button
            size="small"
            class="ctrl-btn"
            :icon="CopyDocument"
            @click="$emit('copy')"
          >
            直链
          </el-button>
        </el-tooltip>

        <el-tooltip content="重新唤起播放器" placement="top">
          <el-button
            size="small"
            class="ctrl-btn"
            :icon="Refresh"
            @click="$emit('re-launch')"
          >
            重播
          </el-button>
        </el-tooltip>
      </div>
    </div>
  </transition>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { Close, DArrowLeft, DArrowRight, CopyDocument, Refresh } from '@element-plus/icons-vue'
import { getEpisodeInfo, type EpisodeInfo } from '@/utils/openlist'

const props = defineProps<{
  visible: boolean
  filename: string
  allFilenames: string[]
  playMode: 'mpv' | 'browser'
}>()

defineEmits<{
  (e: 'close'): void
  (e: 'switch', dir: -1 | 1): void
  (e: 'copy'): void
  (e: 're-launch'): void
}>()

const episodeInfo = computed<EpisodeInfo>(() => {
  if (!props.filename || !props.allFilenames?.length) {
    return { isEpisode: false, hasPrev: false, hasNext: false }
  }
  return getEpisodeInfo(props.allFilenames, props.filename)
})
</script>

<style scoped>
.episode-player-bar {
  position: fixed;
  right: 20px;
  bottom: 20px;
  z-index: 2500;
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-width: 320px;
  max-width: 440px;
  padding: 12px 14px;
  background: var(--el-bg-color-overlay, #ffffff);
  border: 1px solid var(--el-border-color-light, #e4e7ed);
  border-radius: 12px;
  box-shadow: 0 8px 28px rgba(0, 0, 0, 0.16);
  backdrop-filter: blur(12px);
  user-select: none;
}

.bar-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.bar-title-wrap {
  display: flex;
  align-items: center;
  gap: 6px;
  overflow: hidden;
  flex: 1;
  min-width: 0;
}

.ep-tag {
  font-weight: 600;
  flex-shrink: 0;
}

.mode-tag {
  flex-shrink: 0;
}

.file-name {
  font-size: 13px;
  color: var(--el-text-color-regular, #606266);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
}

.close-btn {
  color: var(--el-text-color-secondary, #909399);
  flex-shrink: 0;
  margin-left: 0 !important;
}

.close-btn:hover {
  color: var(--el-color-danger, #f56c6c);
}

/* 控制按钮单行紧凑排列 */
.bar-controls {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
}

.bar-controls :deep(.el-tooltip__trigger) {
  flex: 1;
  display: flex;
  min-width: 0;
}

.ctrl-btn {
  flex: 1;
  width: 100%;
  padding: 7px 4px;
  font-size: 12px;
  margin-left: 0 !important;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  white-space: nowrap;
}

.ctrl-btn :deep(.el-icon) {
  font-size: 13px;
}

/* 动效 */
.episode-bar-fade-enter-active,
.episode-bar-fade-leave-active {
  transition: all 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
}

.episode-bar-fade-enter-from,
.episode-bar-fade-leave-to {
  transform: translateY(20px) scale(0.96);
  opacity: 0;
}

@media (max-width: 640px) {
  .episode-player-bar {
    left: 10px;
    right: 10px;
    bottom: 10px;
    min-width: auto;
    max-width: 100%;
    padding: 10px 12px;
    gap: 8px;
  }

  .ctrl-btn {
    padding: 6px 2px;
    font-size: 12px;
  }
}
</style>
