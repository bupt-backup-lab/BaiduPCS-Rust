import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import axios, { AxiosError, type AxiosAdapter, type AxiosResponse, type InternalAxiosRequestConfig } from 'axios'
import { createPinia, setActivePinia } from 'pinia'

vi.mock('element-plus', () => ({ ElMessage: { error: vi.fn(), success: vi.fn() } }))

import { addWebAuthInterceptor, createApiClient, createRawApiClient } from './client'
import { webAuthApi } from './webAuth'
import { useWebAuthStore } from '@/stores/webAuth'

function respond(config: InternalAxiosRequestConfig, status: number, data: unknown): AxiosResponse {
  return { data, status, statusText: String(status), headers: {}, config }
}

function reject(config: InternalAxiosRequestConfig, status: number, data: unknown): Promise<never> {
  const response = respond(config, status, data)
  return Promise.reject(new AxiosError(`status ${status}`, AxiosError.ERR_BAD_REQUEST, config, null, response))
}

/** 业务接口：只认新令牌，旧令牌一律 419 */
function businessAdapter(config: InternalAxiosRequestConfig): Promise<AxiosResponse> {
  if (config.headers.Authorization !== 'Bearer new-token') {
    return reject(config, 419, { code: 419, message: 'expired' })
  }
  return Promise.resolve(respond(config, 200, { code: 0, message: 'ok', data: { value: 42 } }))
}

describe('Web 认证 419 刷新后重试', () => {
  const originalAdapter = axios.defaults.adapter
  let adapter: ReturnType<typeof vi.fn>
  let refreshSpy: ReturnType<typeof vi.spyOn>

  beforeEach(async () => {
    setActivePinia(createPinia())
    localStorage.clear()
    localStorage.setItem('web_auth_access_token', 'old-token')
    localStorage.setItem('web_auth_refresh_token', 'refresh-token')

    adapter = vi.fn(businessAdapter)
    axios.defaults.adapter = adapter as unknown as AxiosAdapter

    // 刷新接口：模拟后端轮换（每颗 refresh token 只能用一次）
    const used = new Set<string>()
    refreshSpy = vi.spyOn(webAuthApi, 'refreshTokens').mockImplementation(async ({ refresh_token }) => {
      await new Promise((resolve) => setTimeout(resolve, 10))
      if (used.has(refresh_token)) throw new Error('refresh token 已作废')
      used.add(refresh_token)
      const now = Math.floor(Date.now() / 1000)
      return {
        access_token: 'new-token',
        refresh_token: 'new-refresh',
        access_expires_at: now + 3600,
        refresh_expires_at: now + 7200,
      }
    })

    await useWebAuthStore().initialize()
  })

  afterEach(() => {
    useWebAuthStore().clearTokens()
    axios.defaults.adapter = originalAdapter
    refreshSpy.mockRestore()
  })

  it('createApiClient 重试结果只解包一次', async () => {
    const client = createApiClient({ showErrorMessage: false })
    await expect(client.get('/thing')).resolves.toEqual({ value: 42 })
    expect(localStorage.getItem('web_auth_access_token')).toBe('new-token')
    expect(localStorage.getItem('web_auth_refresh_token')).toBe('new-refresh')
    // 原请求 + 重试
    expect(adapter).toHaveBeenCalledTimes(2)
  })

  it('createRawApiClient 重试返回原始响应', async () => {
    const client = createRawApiClient()
    const response = await client.get('/thing')
    expect(response.status).toBe(200)
    expect(response.data).toEqual({ code: 0, message: 'ok', data: { value: 42 } })
  })

  it('重试仍失败时按原始错误处理（保留后端 message）', async () => {
    adapter.mockImplementation((config: InternalAxiosRequestConfig) => {
      if (config.headers.Authorization !== 'Bearer new-token') {
        return reject(config, 419, { code: 419, message: 'expired' })
      }
      return reject(config, 400, { message: '参数错误' })
    })
    const client = createApiClient({ showErrorMessage: false })
    await expect(client.get('/thing')).rejects.toThrow('参数错误')
  })

  it('独立实例（auth.ts / accounts.ts 写法）419 时先刷新再重试', async () => {
    const client = axios.create({ baseURL: '/api/v1' })
    addWebAuthInterceptor(client)
    client.interceptors.response.use((response) => response.data)

    await expect(client.get('/auth/qrcode/status')).resolves.toEqual({ code: 0, message: 'ok', data: { value: 42 } })
    expect(refreshSpy).toHaveBeenCalledTimes(1)
  })

  it('store 尚未初始化时遇到 419：先初始化再刷新，不清掉 localStorage 里的令牌', async () => {
    // 换一个全新的 pinia，模拟请求早于 initialize() 发出
    setActivePinia(createPinia())
    const client = createApiClient({ showErrorMessage: false })

    await expect(client.get('/thing')).resolves.toEqual({ value: 42 })
    expect(refreshSpy).toHaveBeenCalledTimes(1)
    expect(refreshSpy.mock.calls[0][0]).toEqual({ refresh_token: 'refresh-token' })
    expect(localStorage.getItem('web_auth_access_token')).toBe('new-token')
  })

  it('不同客户端 / store 并发刷新只消耗一次 refresh token', async () => {
    const a = createApiClient({ showErrorMessage: false })
    const b = createRawApiClient()
    const store = useWebAuthStore()

    const [ra, rb, refreshed] = await Promise.all([a.get('/x'), b.get('/y'), store.refreshTokens()])

    expect(ra).toEqual({ value: 42 })
    expect(rb.status).toBe(200)
    expect(refreshed).toBe(true)
    expect(refreshSpy).toHaveBeenCalledTimes(1)
    expect(store.isAuthenticated).toBe(true)
    expect(store.accessToken).toBe('new-token')
  })
})
