<template>
  <Teleport to="body">
    <div
        v-if="modelValue"
        ref="menuRef"
        class="file-context-menu"
        role="menu"
        :style="{ left: `${position.left}px`, top: `${position.top}px` }"
        @contextmenu.prevent
    >
      <template v-for="item in items" :key="item.key">
        <div v-if="item.divided" class="file-context-menu__divider" role="separator" />
        <div
            class="file-context-menu__item"
            :class="{ 'is-disabled': item.disabled, 'is-danger': item.danger }"
            role="menuitem"
            :aria-disabled="item.disabled ? 'true' : undefined"
            :title="item.disabled ? item.disabledReason : undefined"
            @click="handleItemClick(item)"
        >
          <el-icon v-if="item.icon" class="file-context-menu__icon">
            <component :is="item.icon" />
          </el-icon>
          <span class="file-context-menu__label">{{ item.label }}</span>
        </div>
      </template>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch, type Component } from 'vue'

export interface ContextMenuItem {
  key: string
  label: string
  icon?: Component
  /** 在该项上方绘制分隔线 */
  divided?: boolean
  disabled?: boolean
  /** 禁用时悬停提示 */
  disabledReason?: string
  /** 危险操作（红色文字） */
  danger?: boolean
}

const props = defineProps<{
  modelValue: boolean
  /** 鼠标指针的视口坐标（clientX / clientY） */
  x: number
  y: number
  items: ContextMenuItem[]
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  select: [key: string]
}>()

/** 菜单与视口边缘保持的最小间距 */
const VIEWPORT_MARGIN = 4

const menuRef = ref<HTMLElement | null>(null)
const position = ref({ left: 0, top: 0 })

function close() {
  if (props.modelValue) emit('update:modelValue', false)
}

function handleItemClick(item: ContextMenuItem) {
  if (item.disabled) return
  close()
  emit('select', item.key)
}

/** 先按指针位置渲染，再根据实际尺寸贴边翻转，避免菜单超出视口 */
async function updatePosition() {
  position.value = { left: props.x, top: props.y }
  await nextTick()
  const el = menuRef.value
  if (!el) return
  const { offsetWidth: w, offsetHeight: h } = el
  const vw = window.innerWidth
  const vh = window.innerHeight
  let left = props.x
  let top = props.y
  if (left + w + VIEWPORT_MARGIN > vw) left = Math.max(VIEWPORT_MARGIN, left - w)
  if (top + h + VIEWPORT_MARGIN > vh) top = Math.max(VIEWPORT_MARGIN, vh - h - VIEWPORT_MARGIN)
  position.value = { left, top }
}

function handlePointerDownOutside(event: Event) {
  const target = event.target as Node | null
  if (target && menuRef.value?.contains(target)) return
  close()
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') close()
}

function addListeners() {
  // capture 阶段监听：即使目标元素阻止了冒泡，也能收起菜单
  document.addEventListener('mousedown', handlePointerDownOutside, true)
  document.addEventListener('touchstart', handlePointerDownOutside, true)
  document.addEventListener('scroll', close, true)
  document.addEventListener('keydown', handleKeydown)
  window.addEventListener('resize', close)
  window.addEventListener('blur', close)
}

function removeListeners() {
  document.removeEventListener('mousedown', handlePointerDownOutside, true)
  document.removeEventListener('touchstart', handlePointerDownOutside, true)
  document.removeEventListener('scroll', close, true)
  document.removeEventListener('keydown', handleKeydown)
  window.removeEventListener('resize', close)
  window.removeEventListener('blur', close)
}

watch(
    () => props.modelValue,
    (visible) => {
      if (visible) {
        addListeners()
      } else {
        removeListeners()
      }
    },
    { immediate: true }
)

// 菜单已打开时在别处再次右键：可见性不变但坐标变了，需要重新定位
watch(
    () => [props.modelValue, props.x, props.y] as const,
    ([visible]) => {
      if (visible) void updatePosition()
    },
    { immediate: true }
)

onBeforeUnmount(removeListeners)
</script>

<style scoped lang="scss">
.file-context-menu {
  position: fixed;
  z-index: 3000;
  min-width: 160px;
  padding: 4px 0;
  background: var(--el-bg-color-overlay, #fff);
  border: 1px solid var(--el-border-color-light, #e4e7ed);
  border-radius: 6px;
  box-shadow: var(--el-box-shadow-light, 0 2px 12px rgba(0, 0, 0, 0.1));
  user-select: none;
}

.file-context-menu__item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 16px;
  height: 34px;
  font-size: 14px;
  color: var(--el-text-color-regular, #606266);
  cursor: pointer;
  white-space: nowrap;

  &:hover {
    background: var(--el-color-primary-light-9, #ecf5ff);
    color: var(--el-color-primary, #409eff);
  }

  &.is-danger {
    color: var(--el-color-danger, #f56c6c);

    &:hover {
      background: var(--el-color-danger-light-9, #fef0f0);
    }
  }

  &.is-disabled {
    color: var(--el-text-color-disabled, #c0c4cc);
    cursor: not-allowed;

    &:hover {
      background: transparent;
    }
  }
}

.file-context-menu__icon {
  font-size: 16px;
}

.file-context-menu__divider {
  height: 1px;
  margin: 4px 0;
  background: var(--el-border-color-lighter, #ebeef5);
}
</style>
