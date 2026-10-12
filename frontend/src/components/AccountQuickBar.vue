<template>
  <div class="account-quick-bar" role="tablist" aria-label="快捷切换账号">
    <button
        v-for="acc in authStore.accounts"
        :key="acc.uid"
        type="button"
        role="tab"
        class="account-quick-bar__item"
        :class="{ 'is-active': acc.uid === authStore.activeUid, 'is-switching': switchingUid === acc.uid }"
        :aria-selected="acc.uid === authStore.activeUid"
        :disabled="switchingUid !== null"
        :title="acc.nickname || acc.username"
        @click="handleSwitch(acc.uid)"
    >
      <VipAvatar
          :src="acc.avatar_url"
          :size="20"
          :vip-type="acc.vip_type"
          :vip-level="acc.vip_level"
      />
      <span class="account-quick-bar__name">{{ acc.nickname || acc.username }}</span>
    </button>
  </div>
</template>

<script setup lang="ts">
/**
 * 多账号横向快捷切换条
 *
 * 由系统设置「界面偏好 → 多账号快捷切换条」开启，嵌入 MainLayout 顶部栏、
 * 位于 AccountSwitcher 左侧；点击即切换，原下拉切换器保持不变。
 */
import { ref } from 'vue'
import { ElMessage } from 'element-plus'
import { useAuthStore } from '@/stores/auth'
import VipAvatar from '@/components/VipAvatar.vue'

const authStore = useAuthStore()
const switchingUid = ref<number | null>(null)

async function handleSwitch(uid: number) {
  if (uid === authStore.activeUid || switchingUid.value !== null) return
  switchingUid.value = uid
  try {
    await authStore.switchAccount(uid)
    const target = authStore.accounts.find((a) => a.uid === uid)
    ElMessage.success(`已切换到 ${target?.nickname || target?.username || `UID:${uid}`}`)
  } catch (err: any) {
    console.error('切换账号失败:', err)
    ElMessage.error(err?.response?.data?.message || err?.message || '切换账号失败')
  } finally {
    switchingUid.value = null
  }
}
</script>

<style scoped>
.account-quick-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  overflow-x: auto;
  scrollbar-width: thin;
}

.account-quick-bar__item {
  display: inline-flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 6px;
  max-width: 160px;
  padding: 3px 10px 3px 4px;
  border: 1px solid var(--el-border-color);
  border-radius: 16px;
  background: var(--el-bg-color);
  color: var(--el-text-color-regular);
  font-size: 13px;
  cursor: pointer;
  transition: border-color 0.15s ease, color 0.15s ease, background-color 0.15s ease;
}

.account-quick-bar__item:hover:not(:disabled) {
  border-color: var(--el-color-primary);
  color: var(--el-color-primary);
}

.account-quick-bar__item.is-active {
  border-color: var(--el-color-primary);
  background: var(--el-color-primary-light-9);
  color: var(--el-color-primary);
  cursor: default;
}

.account-quick-bar__item:disabled:not(.is-active) {
  cursor: wait;
  opacity: 0.6;
}

.account-quick-bar__item.is-switching {
  opacity: 1;
  border-style: dashed;
}

.account-quick-bar__name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
