import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import ElementPlus from 'element-plus'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import CrossAccountTransferDialog from '../CrossAccountTransferDialog.vue'
import type { CrossAccountJob } from '@/api/crossAccount'

const auth = vi.hoisted(() => ({
  activeUid: 1 as number | null,
  accounts: [
    { uid: 1, username: 'alice', nickname: null, avatar_url: null, is_active: true },
    { uid: 2, username: 'bob', nickname: null, avatar_url: null, is_active: false },
  ],
}))

vi.mock('@/stores/auth', () => ({
  useAuthStore: () => auth,
}))

vi.mock('@/utils/responsive', async () => {
  const { ref } = await import('vue')
  return { useIsMobile: () => ref(false) }
})

const api = vi.hoisted(() => ({
  createCrossAccountJob: vi.fn(),
  getCrossAccountJob: vi.fn(),
}))

vi.mock('@/api/crossAccount', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/api/crossAccount')>()),
  createCrossAccountJob: api.createCrossAccountJob,
  getCrossAccountJob: api.getCrossAccountJob,
}))

function makeJob(overrides: Partial<CrossAccountJob> = {}): CrossAccountJob {
  return {
    id: 'job-1',
    mode: 'move',
    source_uid: 1,
    target_uid: 2,
    paths: ['/docs/a.txt'],
    target_dir: '/',
    stage: 'sharing',
    transferred_count: 0,
    total_count: 0,
    error: null,
    failed_stage: null,
    warnings: [],
    source_deleted: false,
    created_at: 0,
    updated_at: 0,
    ...overrides,
  }
}

const file = {
  fs_id: 1,
  path: '/docs/a.txt',
  server_filename: 'a.txt',
  size: 10,
  isdir: 0,
  category: 4,
  server_ctime: 0,
  server_mtime: 0,
  local_ctime: 0,
  local_mtime: 0,
  is_encrypted: false,
  is_encrypted_folder: false,
}

function mountDialog() {
  const wrapper = mount(CrossAccountTransferDialog, {
    attachTo: document.body,
    props: { modelValue: false, files: [file] },
    global: { plugins: [ElementPlus], stubs: { NetdiskFolderPickerModal: true } },
  })
  return wrapper
}

function footerButton(label: string): HTMLButtonElement {
  const btn = Array.from(document.body.querySelectorAll<HTMLButtonElement>('.el-dialog__footer button'))
      .find((b) => b.textContent?.trim() === label)
  expect(btn, `按钮 ${label}`).toBeTruthy()
  return btn!
}

async function flush() {
  for (let i = 0; i < 5; i++) {
    await Promise.resolve()
    await nextTick()
  }
}

beforeEach(() => {
  vi.useFakeTimers()
  api.createCrossAccountJob.mockReset()
  api.getCrossAccountJob.mockReset()
})

afterEach(() => {
  vi.useRealTimers()
  document.body.innerHTML = ''
})

describe('CrossAccountTransferDialog', () => {
  it('只有一个其他账号时自动选中，提交后轮询到完成并发出 finished', async () => {
    const wrapper = mountDialog()
    await wrapper.setProps({ modelValue: true })
    await flush()

    // @ts-expect-error setup 内部状态
    expect(wrapper.vm.targetUid).toBe(2)
    // @ts-expect-error setup 内部状态
    wrapper.vm.mode = 'move'
    await nextTick()

    api.createCrossAccountJob.mockResolvedValue(makeJob())
    api.getCrossAccountJob
        .mockResolvedValueOnce(makeJob({ stage: 'transferring', transferred_count: 1, total_count: 2 }))
        .mockResolvedValueOnce(makeJob({ stage: 'completed', source_deleted: true }))

    footerButton('开始移动').click()
    await flush()
    expect(api.createCrossAccountJob).toHaveBeenCalledWith({
      source_uid: 1,
      target_uid: 2,
      paths: ['/docs/a.txt'],
      target_dir: '/',
      mode: 'move',
    })

    await vi.advanceTimersByTimeAsync(1000)
    await flush()
    expect(document.body.textContent).toContain('（1 / 2）')

    await vi.advanceTimersByTimeAsync(1000)
    await flush()
    expect(wrapper.emitted('finished')?.[0]?.[0]).toMatchObject({ stage: 'completed', source_deleted: true })
    expect(document.body.textContent).toContain('已移动到 bob 的 /')

    // 结束后不再轮询
    await vi.advanceTimersByTimeAsync(5000)
    expect(api.getCrossAccountJob).toHaveBeenCalledTimes(2)

    wrapper.unmount()
  })

  it('连续轮询失败后放弃跟踪，再次打开回到表单', async () => {
    const wrapper = mountDialog()
    await wrapper.setProps({ modelValue: true })
    await flush()

    api.createCrossAccountJob.mockResolvedValue(makeJob())
    api.getCrossAccountJob.mockRejectedValue(new Error('network down'))

    footerButton('开始复制').click()
    await flush()
    for (let i = 0; i < 10; i++) {
      await vi.advanceTimersByTimeAsync(1000)
      await flush()
    }
    expect(document.body.textContent).toContain('无法获取任务状态：network down')
    expect(footerButton('完成')).toBeTruthy()

    await wrapper.setProps({ modelValue: false })
    await wrapper.setProps({ modelValue: true })
    await flush()
    expect(footerButton('开始复制')).toBeTruthy()

    wrapper.unmount()
  })

  it('失败时展示错误与告警', async () => {
    const wrapper = mountDialog()
    await wrapper.setProps({ modelValue: true })
    await flush()

    api.createCrossAccountJob.mockResolvedValue(makeJob({ mode: 'copy' }))
    api.getCrossAccountJob.mockResolvedValueOnce(
        makeJob({
          mode: 'copy',
          stage: 'failed',
          failed_stage: 'transferring',
          error: '转存失败：网盘空间不足',
          warnings: ['撤销临时分享失败'],
        }),
    )

    footerButton('开始复制').click()
    await flush()
    await vi.advanceTimersByTimeAsync(1000)
    await flush()

    expect(document.body.textContent).toContain('转存失败：网盘空间不足')
    expect(document.body.textContent).toContain('撤销临时分享失败')
    // @ts-expect-error setup 内部状态
    expect(wrapper.vm.activeStep).toBe(1)
    expect(wrapper.emitted('finished')).toHaveLength(1)

    wrapper.unmount()
  })
})
