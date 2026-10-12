/**
 * 跨账号复制 / 移动 API
 *
 * 后端做法：源账号建临时私密分享 → 目标账号转存 → 立即撤销分享；
 * 移动模式在转存完全成功后删除源文件。任务在后台执行，前端轮询任务状态。
 *
 *   - POST /api/v1/cross-account/jobs      创建任务
 *   - GET  /api/v1/cross-account/jobs/:id  查询任务
 *   - GET  /api/v1/cross-account/jobs      列出任务（含最近一小时内结束的）
 */

import { createApiClient } from './client'

// 错误由调用方在对话框内展示，不走全局弹窗
const silentClient = createApiClient({ showErrorMessage: false })

export type CrossAccountMode = 'copy' | 'move'

export type CrossAccountStage =
    | 'checking'
    | 'sharing'
    | 'transferring'
    | 'revoking'
    | 'verifying'
    | 'deleting_source'
    | 'completed'
    | 'failed'

export interface CrossAccountJob {
  id: string
  mode: CrossAccountMode
  source_uid: number
  target_uid: number
  paths: string[]
  target_dir: string
  stage: CrossAccountStage
  transferred_count: number
  total_count: number
  /** 致命错误（stage=failed） */
  error: string | null
  /** 失败发生的阶段 */
  failed_stage: CrossAccountStage | null
  /** 非致命问题：部分未转存、撤销分享失败、删除源文件失败等 */
  warnings: string[]
  source_deleted: boolean
  created_at: number
  updated_at: number
}

export interface CreateCrossAccountJobRequest {
  /** 缺省为当前活跃账号 */
  source_uid?: number
  target_uid: number
  paths: string[]
  target_dir: string
  mode: CrossAccountMode
}

export const CROSS_ACCOUNT_STAGE_LABELS: Record<CrossAccountStage, string> = {
  checking: '正在检查同名冲突',
  sharing: '正在创建临时分享',
  transferring: '目标账号转存中',
  revoking: '正在撤销临时分享',
  verifying: '正在核对目标文件',
  deleting_source: '正在删除源文件',
  completed: '已完成',
  failed: '失败',
}

export function isCrossAccountJobFinished(job: CrossAccountJob): boolean {
  return job.stage === 'completed' || job.stage === 'failed'
}

export async function createCrossAccountJob(req: CreateCrossAccountJobRequest): Promise<CrossAccountJob> {
  return silentClient.post('/cross-account/jobs', req)
}

export async function getCrossAccountJob(id: string): Promise<CrossAccountJob> {
  return silentClient.get(`/cross-account/jobs/${id}`)
}

export async function listCrossAccountJobs(): Promise<CrossAccountJob[]> {
  return silentClient.get('/cross-account/jobs')
}
