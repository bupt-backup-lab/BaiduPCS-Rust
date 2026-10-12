<template>
  <el-dialog
      :model-value="modelValue"
      title="目标文件夹中已有同名项"
      width="480px"
      append-to-body
      :before-close="handleUserClose"
      @update:model-value="(v: boolean) => emit('update:modelValue', v)"
  >
    <div class="conflict-body">
      <p class="conflict-summary">
        以下 {{ conflicts.length }} 个项目在目标文件夹中已存在，请选择处理方式：
      </p>
      <ul class="conflict-list">
        <li v-for="item in conflicts" :key="item.path" class="conflict-item" :title="item.path">
          <el-icon class="conflict-icon" :color="item.isdir ? '#e6a23c' : '#409eff'">
            <Folder v-if="item.isdir" />
            <Document v-else />
          </el-icon>
          <span class="conflict-name">{{ item.name }}</span>
        </li>
      </ul>
      <p v-if="doneCount > 0" class="conflict-done">
        其余 {{ doneCount }} 项已{{ actionText }}完成。
      </p>
      <p v-if="hasFolder" class="conflict-hint">
        同名文件夹会被整个替换而不是合并，为避免误删，含文件夹时不提供"覆盖"。
      </p>
    </div>
    <template #footer>
      <el-button @click="choose('skip')">跳过</el-button>
      <el-button
          type="danger"
          plain
          :disabled="hasFolder"
          @click="choose('overwrite')"
      >
        覆盖
      </el-button>
      <el-button type="primary" @click="choose('newcopy')">保留两者</el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { Document, Folder } from '@element-plus/icons-vue'

export interface ConflictEntry {
  /** 源路径 */
  path: string
  /** 展示名称 */
  name: string
  isdir: boolean
}

/** 用户选择：保留两者 / 覆盖 / 跳过（点 × / Esc / 遮罩关闭视为跳过） */
export type ConflictChoice = 'newcopy' | 'overwrite' | 'skip'

const props = defineProps<{
  modelValue: boolean
  operation: 'copy' | 'move'
  conflicts: ConflictEntry[]
  /** 本次已完成（不冲突）的项数 */
  doneCount: number
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  resolve: [choice: ConflictChoice]
}>()

const actionText = computed(() => (props.operation === 'copy' ? '复制' : '移动'))
const hasFolder = computed(() => props.conflicts.some((c) => c.isdir))

function choose(choice: ConflictChoice) {
  emit('update:modelValue', false)
  emit('resolve', choice)
}

// before-close 只在用户主动关闭时调用；父组件改 modelValue 关闭时不会触发，避免重复 resolve
function handleUserClose(done: () => void) {
  done()
  choose('skip')
}
</script>

<style scoped lang="scss">
.conflict-summary {
  margin: 0 0 8px;
  color: var(--el-text-color-regular);
}

.conflict-list {
  list-style: none;
  margin: 0;
  padding: 4px 0;
  max-height: 220px;
  overflow-y: auto;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
}

.conflict-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
}

.conflict-icon {
  flex-shrink: 0;
}

.conflict-name {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.conflict-done {
  margin: 10px 0 0;
  color: var(--el-color-success);
}

.conflict-hint {
  margin: 10px 0 0;
  font-size: 13px;
  color: var(--el-text-color-secondary);
}
</style>
