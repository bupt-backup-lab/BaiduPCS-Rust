import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import ElementPlus, { ElMessage, ElNotification } from 'element-plus'
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

const router = vi.hoisted(() => ({ push: vi.fn() }))

vi.mock('vue-router', () => ({
  useRouter: () => router,
}))

vi.mock('@/api/config', () => ({
  getConfig: vi.fn().mockResolvedValue({
    download: { download_dir: 'downloads', default_directory: 'D:/dl', ask_each_time: false },
  }),
  setDefaultDownloadAccount: vi.fn().mockResolvedValue('ok'),
  setDefaultDownloadDir: vi.fn(),
  updateRecentDirDebounced: vi.fn(),
}))

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
    local_download_path: null,
    transfer_task_id: null,
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
    global: { plugins: [ElementPlus], stubs: { NetdiskFolderPickerModal: true, FilePickerModal: true } },
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
  router.push.mockReset()
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

  it('下载模式：默认填入默认下载目录，提交本地目录，完成后可跳转下载管理', async () => {
    const wrapper = mountDialog()
    await wrapper.setProps({ initialMode: 'download' })
    await wrapper.setProps({ modelValue: true })
    await flush()

    expect(document.body.querySelector('.el-dialog__title')?.textContent).toBe('用其他账号下载')
    // @ts-expect-error setup 内部状态
    expect(wrapper.vm.localDownloadPath).toBe('D:/dl')

    api.createCrossAccountJob.mockResolvedValue(
        makeJob({ mode: 'download', target_dir: '', local_download_path: 'D:/dl' }),
    )
    api.getCrossAccountJob
        .mockResolvedValueOnce(
            makeJob({ mode: 'download', target_dir: '', local_download_path: 'D:/dl', stage: 'transferring' }),
        )
        .mockResolvedValueOnce(
            makeJob({
              mode: 'download',
              target_dir: '',
              local_download_path: 'D:/dl',
              stage: 'completed',
              transfer_task_id: 't-1',
            }),
        )

    footerButton('开始下载').click()
    await flush()
    expect(api.createCrossAccountJob).toHaveBeenCalledWith({
      source_uid: 1,
      target_uid: 2,
      paths: ['/docs/a.txt'],
      local_download_path: 'D:/dl',
      mode: 'download',
    })

    await vi.advanceTimersByTimeAsync(1000)
    await flush()
    expect(document.body.textContent).toContain('下载账号转存到临时目录中')

    await vi.advanceTimersByTimeAsync(1000)
    await flush()
    expect(document.body.textContent).toContain('已由 bob 开始下载到 D:/dl')
    // @ts-expect-error setup 内部状态
    expect(wrapper.vm.activeStep).toBe(3)

    footerButton('查看下载').click()
    await flush()
    expect(router.push).toHaveBeenCalledWith('/downloads')
    expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual([false])

    wrapper.unmount()
  })

  it('下载模式未选本地目录时不能提交', async () => {
    const { getConfig } = await import('@/api/config')
    vi.mocked(getConfig).mockResolvedValueOnce({ download: { download_dir: '', ask_each_time: false } } as never)
    const wrapper = mountDialog()
    await wrapper.setProps({ initialMode: 'download' })
    await wrapper.setProps({ modelValue: true })
    await flush()

    expect(footerButton('开始下载').disabled).toBe(true)
    // @ts-expect-error setup 内部方法
    wrapper.vm.handleConfirmDownload({ path: 'E:/x', setAsDefault: false })
    await flush()
    expect(footerButton('开始下载').disabled).toBe(false)

    wrapper.unmount()
  })

  it('下载模式后台运行：结束通知可点击跳转下载管理；转存前失败则不可点', async () => {
    const success = vi.spyOn(ElNotification, 'success').mockImplementation(() => ({ close: () => {} }))
    const error = vi.spyOn(ElNotification, 'error').mockImplementation(() => ({ close: () => {} }))
    const dl = { mode: 'download' as const, target_dir: '', local_download_path: 'D:/dl' }

    async function runInBackground(final: CrossAccountJob) {
      const wrapper = mountDialog()
      await wrapper.setProps({ initialMode: 'download' })
      await wrapper.setProps({ modelValue: true })
      await flush()
      api.createCrossAccountJob.mockResolvedValue(makeJob(dl))
      api.getCrossAccountJob.mockResolvedValueOnce(final)
      footerButton('开始下载').click()
      await flush()
      footerButton('后台运行').click()
      await wrapper.setProps({ modelValue: false })
      await vi.advanceTimersByTimeAsync(1000)
      await flush()
      wrapper.unmount()
    }

    await runInBackground(makeJob({ ...dl, stage: 'completed' }))
    const okOpts = success.mock.calls[0][0] as { message: string; onClick?: () => void }
    expect(okOpts.message).toContain('点击查看下载')
    okOpts.onClick?.()
    expect(router.push).toHaveBeenCalledWith('/downloads')

    // 开始下载后失败：可点击
    await runInBackground(makeJob({ ...dl, stage: 'failed', failed_stage: 'revoking', error: '下载失败' }))
    expect((error.mock.calls[0][0] as { onClick?: unknown }).onClick).toBeTypeOf('function')

    // 转存阶段失败：下载管理里没有任务，不可点击
    await runInBackground(makeJob({ ...dl, stage: 'failed', failed_stage: 'transferring', error: '网盘空间不足' }))
    const failOpts = error.mock.calls[1][0] as { message: string; onClick?: unknown }
    expect(failOpts.onClick).toBeUndefined()
    expect(failOpts.message).toBe('网盘空间不足')

    success.mockRestore()
    error.mockRestore()
  })

  describe('下载账号预选', () => {
    const baseAccounts = auth.accounts

    afterEach(() => {
      auth.accounts = baseAccounts
    })

    function useAccounts(vipTypes: Record<number, number>) {
      auth.accounts = [1, 2, 3, 4].map((uid) => ({
        uid,
        username: `u${uid}`,
        nickname: null,
        avatar_url: null,
        is_active: uid === 1,
        vip_type: vipTypes[uid] ?? 0,
      })) as typeof baseAccounts
    }

    async function openDownload(defaultUid?: number | null) {
      const { getConfig } = await import('@/api/config')
      vi.mocked(getConfig).mockResolvedValueOnce({
        download: { download_dir: 'D:/dl', ask_each_time: false, default_download_account_uid: defaultUid },
      } as never)
      const wrapper = mountDialog()
      await wrapper.setProps({ initialMode: 'download' })
      await wrapper.setProps({ modelValue: true })
      await flush()
      return wrapper
    }

    it('没有默认账号时预选第一个非源账号的 SVIP', async () => {
      useAccounts({ 1: 2, 3: 2, 4: 2 })
      const wrapper = await openDownload()
      // @ts-expect-error setup 内部状态
      expect(wrapper.vm.targetUid).toBe(3)
      wrapper.unmount()
    })

    it('设好的默认下载账号优先于 SVIP；默认账号是源账号时退回 SVIP', async () => {
      useAccounts({ 3: 2 })
      let wrapper = await openDownload(4)
      // @ts-expect-error setup 内部状态
      expect(wrapper.vm.targetUid).toBe(4)
      // @ts-expect-error setup 内部状态
      expect(wrapper.vm.isDefaultAccount).toBe(true)
      wrapper.unmount()

      wrapper = await openDownload(1)
      // @ts-expect-error setup 内部状态
      expect(wrapper.vm.targetUid).toBe(3)
      wrapper.unmount()
    })

    it('多个候选且无 SVIP、无默认时不预选；复制模式不按 SVIP 预选', async () => {
      useAccounts({})
      const wrapper = await openDownload()
      // @ts-expect-error setup 内部状态
      expect(wrapper.vm.targetUid).toBeNull()
      wrapper.unmount()

      useAccounts({ 3: 2 })
      const copy = mountDialog()
      await copy.setProps({ modelValue: true })
      await flush()
      // @ts-expect-error setup 内部状态
      expect(copy.vm.targetUid).toBeNull()
      copy.unmount()
    })

    it('勾选 / 取消「设为默认下载账号」会保存到后端', async () => {
      useAccounts({})
      const { setDefaultDownloadAccount } = await import('@/api/config')
      vi.mocked(setDefaultDownloadAccount).mockClear()
      const wrapper = await openDownload()
      // @ts-expect-error setup 内部状态
      wrapper.vm.targetUid = 2
      await flush()

      const checkbox = () => document.body.querySelector<HTMLInputElement>('.cross-account__default input')!
      expect(checkbox().disabled).toBe(false)
      checkbox().click()
      await flush()
      expect(setDefaultDownloadAccount).toHaveBeenLastCalledWith(2)
      // @ts-expect-error setup 内部状态
      expect(wrapper.vm.isDefaultAccount).toBe(true)

      checkbox().click()
      await flush()
      expect(setDefaultDownloadAccount).toHaveBeenLastCalledWith(null)
      // @ts-expect-error setup 内部状态
      expect(wrapper.vm.isDefaultAccount).toBe(false)
      wrapper.unmount()
    })

    it('下载配置首次加载失败：再次以下载模式打开会重试并套用默认账号', async () => {
      useAccounts({})
      const { getConfig } = await import('@/api/config')
      vi.mocked(getConfig).mockRejectedValueOnce(new Error('boom'))
      const wrapper = mountDialog()
      await wrapper.setProps({ initialMode: 'download' })
      await wrapper.setProps({ modelValue: true })
      await flush()
      // @ts-expect-error setup 内部状态
      expect(wrapper.vm.targetUid).toBeNull()

      vi.mocked(getConfig).mockResolvedValueOnce({
        download: { download_dir: 'D:/dl', ask_each_time: false, default_download_account_uid: 3 },
      } as never)
      await wrapper.setProps({ modelValue: false })
      await wrapper.setProps({ modelValue: true })
      await flush()
      // @ts-expect-error setup 内部状态
      expect(wrapper.vm.targetUid).toBe(3)
      // @ts-expect-error setup 内部状态
      expect(wrapper.vm.localDownloadPath).toBe('D:/dl')
      wrapper.unmount()
    })

    it('下载配置加载失败时勾选默认账号仍保持勾选', async () => {
      useAccounts({})
      const { getConfig, setDefaultDownloadAccount } = await import('@/api/config')
      vi.mocked(getConfig).mockRejectedValueOnce(new Error('boom'))
      const wrapper = mountDialog()
      await wrapper.setProps({ initialMode: 'download' })
      await wrapper.setProps({ modelValue: true })
      await flush()
      // @ts-expect-error setup 内部状态
      wrapper.vm.targetUid = 2
      await flush()

      document.body.querySelector<HTMLInputElement>('.cross-account__default input')!.click()
      await flush()
      expect(setDefaultDownloadAccount).toHaveBeenLastCalledWith(2)
      // @ts-expect-error setup 内部状态
      expect(wrapper.vm.isDefaultAccount).toBe(true)
      wrapper.unmount()
    })

    it('配置请求途中保存了默认账号，后到的旧配置不会覆盖', async () => {
      useAccounts({})
      const { getConfig, setDefaultDownloadAccount } = await import('@/api/config')
      let resolveConfig!: (v: unknown) => void
      vi.mocked(getConfig).mockReturnValueOnce(new Promise((r) => (resolveConfig = r)) as never)
      const wrapper = mountDialog()
      await wrapper.setProps({ initialMode: 'download' })
      await wrapper.setProps({ modelValue: true })
      await flush()
      // @ts-expect-error setup 内部状态
      wrapper.vm.targetUid = 2
      await flush()

      document.body.querySelector<HTMLInputElement>('.cross-account__default input')!.click()
      await flush()
      expect(setDefaultDownloadAccount).toHaveBeenLastCalledWith(2)

      resolveConfig({ download: { download_dir: 'D:/dl', ask_each_time: false, default_download_account_uid: 4 } })
      await flush()
      // @ts-expect-error setup 内部状态
      expect(wrapper.vm.isDefaultAccount).toBe(true)
      // @ts-expect-error setup 内部状态
      expect(wrapper.vm.targetUid).toBe(2)
      // @ts-expect-error setup 内部状态
      expect(wrapper.vm.localDownloadPath).toBe('D:/dl')
      wrapper.unmount()
    })

    it('保存默认账号失败时不重复弹错，勾选态不变', async () => {
      useAccounts({})
      const { setDefaultDownloadAccount } = await import('@/api/config')
      vi.mocked(setDefaultDownloadAccount).mockRejectedValueOnce(new Error('账号不存在'))
      const msg = vi.spyOn(ElMessage, 'error')
      const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {})
      const wrapper = await openDownload()
      // @ts-expect-error setup 内部状态
      wrapper.vm.targetUid = 2
      await flush()

      document.body.querySelector<HTMLInputElement>('.cross-account__default input')!.click()
      await flush()
      expect(msg).not.toHaveBeenCalled()
      // @ts-expect-error setup 内部状态
      expect(wrapper.vm.isDefaultAccount).toBe(false)
      msg.mockRestore()
      consoleError.mockRestore()
      wrapper.unmount()
    })
  })
})
