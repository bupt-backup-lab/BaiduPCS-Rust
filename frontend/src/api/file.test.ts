import { beforeEach, describe, expect, it, vi } from 'vitest'

// file.ts 在模块加载时 axios.create()；拦截出一个可控的 get
const http = vi.hoisted(() => ({ get: vi.fn() }))

vi.mock('axios', () => ({
  default: {
    create: () => ({
      get: http.get,
      interceptors: { request: { use: vi.fn() }, response: { use: vi.fn() } },
    }),
  },
}))

import { isNotFoundError, locateNetdiskFile } from './file'

function page(paths: string[], hasMore: boolean) {
  return { data: { code: 0, message: 'ok', data: { list: paths.map((path) => ({ path })), has_more: hasMore, page: 1 } } }
}

describe('isNotFoundError', () => {
  it('只把 errno -9 识别为目录不存在', () => {
    expect(isNotFoundError(new Error('获取文件列表失败: API error -9: '))).toBe(true)
    expect(isNotFoundError(new Error('获取文件列表失败: errno=-9, errmsg='))).toBe(true)
    expect(isNotFoundError(new Error('API error -90: '))).toBe(false)
    expect(isNotFoundError(new Error('网络错误'))).toBe(false)
  })
})

describe('locateNetdiskFile', () => {
  beforeEach(() => {
    http.get.mockReset()
  })

  it('翻页找到文件，并按指定账号查询', async () => {
    http.get
        .mockResolvedValueOnce(page(['/up/a.txt'], true))
        .mockResolvedValueOnce(page(['/up/b.txt'], false))

    await expect(locateNetdiskFile('/up/b.txt', 42)).resolves.toBe('found')
    expect(http.get).toHaveBeenCalledTimes(2)
    expect(http.get.mock.calls[0][1].params).toMatchObject({ dir: '/up', page: 1, uid: 42 })
    expect(http.get.mock.calls[1][1].params).toMatchObject({ dir: '/up', page: 2 })
  })

  it('目录存在但没有该文件', async () => {
    http.get.mockResolvedValueOnce(page(['/up/a.txt'], false))
    await expect(locateNetdiskFile('/up/b.txt')).resolves.toBe('file_not_found')
  })

  it('目录不存在（errno -9）', async () => {
    http.get.mockResolvedValueOnce({ data: { code: 500, message: '获取文件列表失败: API error -9: ' } })
    await expect(locateNetdiskFile('/测试2-1/dummy_1.txt')).resolves.toBe('dir_not_found')
  })

  it('其他错误向上抛出', async () => {
    http.get.mockResolvedValueOnce({ data: { code: 500, message: '获取文件列表失败: API error -6: ' } })
    await expect(locateNetdiskFile('/up/b.txt')).rejects.toThrow('API error -6')
  })
})
