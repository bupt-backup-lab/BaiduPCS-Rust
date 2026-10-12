import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import ElementPlus from 'element-plus'
import { afterEach, describe, expect, it } from 'vitest'
import FileConflictDialog, { type ConflictEntry } from '../FileConflictDialog.vue'

function mountDialog(conflicts: ConflictEntry[], doneCount = 0) {
  return mount(FileConflictDialog, {
    attachTo: document.body,
    props: { modelValue: true, operation: 'copy', conflicts, doneCount },
    global: { plugins: [ElementPlus] },
  })
}

function footerButton(label: string): HTMLButtonElement {
  const btn = Array.from(document.body.querySelectorAll<HTMLButtonElement>('.el-dialog__footer button'))
      .find((b) => b.textContent?.trim() === label)
  expect(btn, `按钮 ${label}`).toBeTruthy()
  return btn!
}

afterEach(() => {
  document.body.innerHTML = ''
})

describe('FileConflictDialog', () => {
  it('列出冲突项与已完成数量；只有文件冲突时可覆盖', async () => {
    const wrapper = mountDialog([{ path: '/a/f.txt', name: 'f.txt', isdir: false }], 2)
    await nextTick()

    const names = Array.from(document.body.querySelectorAll('.conflict-name')).map((el) => el.textContent)
    expect(names).toEqual(['f.txt'])
    expect(document.body.querySelector('.conflict-done')?.textContent).toContain('其余 2 项已复制完成')
    expect(footerButton('覆盖').disabled).toBe(false)

    footerButton('覆盖').click()
    await nextTick()
    expect(wrapper.emitted('resolve')).toEqual([['overwrite']])
    expect(wrapper.emitted('update:modelValue')).toEqual([[false]])

    wrapper.unmount()
  })

  it('含文件夹冲突时禁用覆盖并给出原因', async () => {
    const wrapper = mountDialog([
      { path: '/a/f.txt', name: 'f.txt', isdir: false },
      { path: '/a/dir', name: 'dir', isdir: true },
    ])
    await nextTick()

    expect(footerButton('覆盖').disabled).toBe(true)
    expect(document.body.querySelector('.conflict-hint')?.textContent).toContain('整个替换')

    footerButton('保留两者').click()
    await nextTick()
    expect(wrapper.emitted('resolve')).toEqual([['newcopy']])

    wrapper.unmount()
  })

  it('点击跳过或关闭弹窗都视为跳过', async () => {
    const wrapper = mountDialog([{ path: '/a/f.txt', name: 'f.txt', isdir: false }])
    await nextTick()

    footerButton('跳过').click()
    await nextTick()
    expect(wrapper.emitted('resolve')).toEqual([['skip']])

    await wrapper.setProps({ modelValue: true })
    ;(document.body.querySelector('.el-dialog__headerbtn') as HTMLElement).click()
    await nextTick()
    expect(wrapper.emitted('resolve')).toEqual([['skip'], ['skip']])

    wrapper.unmount()
  })
})
