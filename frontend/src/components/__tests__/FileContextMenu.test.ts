import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import ElementPlus from 'element-plus'
import { afterEach, describe, expect, it } from 'vitest'
import FileContextMenu, { type ContextMenuItem } from '../FileContextMenu.vue'

const items: ContextMenuItem[] = [
  { key: 'download', label: '下载' },
  { key: 'rename', label: '重命名', disabled: true, disabledReason: '加密文件/文件夹不支持此操作' },
  { key: 'delete', label: '删除', divided: true, danger: true },
]

function mountMenu(props: Partial<{ modelValue: boolean; x: number; y: number }> = {}) {
  return mount(FileContextMenu, {
    attachTo: document.body,
    props: {
      modelValue: true,
      x: 100,
      y: 120,
      items,
      ...props,
    },
    global: { plugins: [ElementPlus] },
  })
}

function menuEl(): HTMLElement | null {
  return document.body.querySelector('.file-context-menu')
}

afterEach(() => {
  document.body.innerHTML = ''
})

describe('FileContextMenu', () => {
  it('按指针坐标渲染菜单项、分隔线与禁用态', async () => {
    const wrapper = mountMenu()
    await nextTick()
    await nextTick()

    const menu = menuEl()!
    expect(menu).not.toBeNull()
    expect(menu.style.left).toBe('100px')
    expect(menu.style.top).toBe('120px')

    const labels = Array.from(menu.querySelectorAll('.file-context-menu__label')).map((el) => el.textContent)
    expect(labels).toEqual(['下载', '重命名', '删除'])
    expect(menu.querySelectorAll('.file-context-menu__divider')).toHaveLength(1)

    const rename = menu.querySelectorAll<HTMLElement>('.file-context-menu__item')[1]
    expect(rename.classList.contains('is-disabled')).toBe(true)
    expect(rename.getAttribute('title')).toBe('加密文件/文件夹不支持此操作')

    wrapper.unmount()
  })

  it('点击可用项触发 select 并关闭；点击禁用项无反应', async () => {
    const wrapper = mountMenu()
    await nextTick()

    const [download, rename] = Array.from(menuEl()!.querySelectorAll<HTMLElement>('.file-context-menu__item'))

    rename.click()
    await nextTick()
    expect(wrapper.emitted('select')).toBeUndefined()
    expect(menuEl()).not.toBeNull()

    download.click()
    await nextTick()
    expect(wrapper.emitted('select')).toEqual([['download']])
    expect(wrapper.emitted('update:modelValue')).toEqual([[false]])

    wrapper.unmount()
  })

  it('菜单外按下鼠标或按 Esc 时关闭，菜单内按下不关闭', async () => {
    const wrapper = mountMenu()
    await nextTick()

    menuEl()!.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }))
    expect(wrapper.emitted('update:modelValue')).toBeUndefined()

    document.body.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }))
    expect(wrapper.emitted('update:modelValue')).toEqual([[false]])

    await wrapper.setProps({ modelValue: true })
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
    expect(wrapper.emitted('update:modelValue')).toEqual([[false], [false]])

    wrapper.unmount()
  })

  it('靠近视口右下角时向左上翻转，不超出视口', async () => {
    const origW = window.innerWidth
    const origH = window.innerHeight
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 800 })
    Object.defineProperty(window, 'innerHeight', { configurable: true, value: 600 })
    const widthDesc = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetWidth')
    const heightDesc = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetHeight')
    Object.defineProperty(HTMLElement.prototype, 'offsetWidth', { configurable: true, get: () => 160 })
    Object.defineProperty(HTMLElement.prototype, 'offsetHeight', { configurable: true, get: () => 200 })

    try {
      const wrapper = mountMenu({ x: 790, y: 590 })
      await nextTick()
      await nextTick()

      const menu = menuEl()!
      expect(menu.style.left).toBe(`${790 - 160}px`)
      expect(menu.style.top).toBe(`${600 - 200 - 4}px`)
      wrapper.unmount()
    } finally {
      Object.defineProperty(window, 'innerWidth', { configurable: true, value: origW })
      Object.defineProperty(window, 'innerHeight', { configurable: true, value: origH })
      if (widthDesc) Object.defineProperty(HTMLElement.prototype, 'offsetWidth', widthDesc)
      if (heightDesc) Object.defineProperty(HTMLElement.prototype, 'offsetHeight', heightDesc)
    }
  })
})
