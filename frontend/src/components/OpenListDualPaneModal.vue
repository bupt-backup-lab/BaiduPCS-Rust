<template>
  <teleport to="body">
    <div v-if="visible" class="omni-overlay" @keydown.esc="handleClose">
      <!-- 主容器 -->
      <div class="omni-modal" :data-tab="activeMobileTab">
        <!-- 移动端顶部 Tab 切换器 -->
        <div class="omni-mtabs">
          <button
            class="omni-mtab"
            :class="{ active: activeMobileTab === 'left' }"
            @click="activeMobileTab = 'left'"
          >
            <b>A</b>
            <span class="tab-name">{{ getPaneTabName('left') }}</span>
            <i v-if="selectedCount('left') > 0" class="omni-badge">{{ selectedCount('left') }}</i>
          </button>
          <button
            class="omni-mtab"
            :class="{ active: activeMobileTab === 'right' }"
            @click="activeMobileTab = 'right'"
          >
            <b>B</b>
            <span class="tab-name">{{ getPaneTabName('right') }}</span>
            <i v-if="selectedCount('right') > 0" class="omni-badge">{{ selectedCount('right') }}</i>
          </button>
          <button class="omni-mexit" title="退出并刷新" @click="handleClose">✕</button>
        </div>

        <!-- 双栏主体内容 -->
        <div class="omni-body">
          <!-- 面板 A (左) -->
          <div class="omni-panel omni-panel-left">
            <div class="omni-list-header">
              <b>面板 A</b>
              <div class="omni-tools">
                <button class="tool-btn" @click="renderPane('left', true)">🔄 刷新</button>
                <button class="tool-btn" @click="handleMkdir('left')">➕ 新建文件夹</button>
                <button class="tool-btn" @click="syncPath('left', 'right')">📍 同步给右侧</button>
              </div>
            </div>

            <div class="omni-path-container">
              <span class="omni-crumb" @click="navigateToPath('left', '/')">🏠 根目录</span>
              <template v-for="(crumb, idx) in getBreadcrumbs('left')" :key="crumb.fullPath">
                <span class="omni-crumb-sep">/</span>
                <span class="omni-crumb" @click="navigateToPath('left', crumb.fullPath)">{{ crumb.name }}</span>
              </template>
            </div>

            <div class="omni-list" :class="{ 'has-bottom-sheet': ctxMenu.visible && isMobile }">
              <div v-if="panes.left.loading" class="omni-empty">正在读取...</div>
              <div v-else-if="panes.left.error" class="omni-empty text-error">
                加载失败: {{ panes.left.error }}
              </div>
              <template v-else>
                <!-- 返回上一级 -->
                <div v-if="panes.left.path !== '/'" class="omni-item omni-item-up" @click="goUp('left')">
                  <span style="margin-right: 12px; width: 16px;"></span>
                  ⬅️ <b>返回上一级</b>
                </div>

                <!-- 文件列表 -->
                <div
                  v-for="item in visibleItems('left')"
                  :key="item.name"
                  class="omni-item data-row"
                  :class="{ 'omni-item-active': ctxMenu.visible && ctxMenu.side === 'left' && ctxMenu.item?.name === item.name }"
                  @click="handleItemClick('left', item, $event)"
                  @contextmenu.prevent="openContextMenu('left', item, $event)"
                >
                  <input
                    type="checkbox"
                    class="omni-cb"
                    :checked="isItemSelected('left', item.name)"
                    @click.stop="toggleItemSelect('left', item.name)"
                  />
                  <span class="omni-item-title">
                    {{ item.is_dir ? '📁 ' : '📄 ' }}{{ item.name }}
                  </span>
                  <span class="omni-more" @click.stop="openContextMenuMobile('left', item, $event)">⋯</span>
                </div>
              </template>
            </div>

            <!-- 状态筛选栏 -->
            <div class="omni-status-bar">
              <div class="omni-filter-row">
                <input
                  v-model="panes.left.filterText"
                  type="text"
                  class="omni-filter-input"
                  :placeholder="getFilterPlaceholder(panes.left.filterMode)"
                  @keydown.enter="applyFilter('left')"
                />
                <div class="omni-filter-modes">
                  <label><input v-model="panes.left.filterMode" type="radio" value="regex" /> 正则</label>
                  <label><input v-model="panes.left.filterMode" type="radio" value="keyword" /> 关键字</label>
                  <label><input v-model="panes.left.filterMode" type="radio" value="number" /> 选前N个</label>
                </div>
              </div>

              <div class="omni-tag-row">
                <label
                  v-if="panes.left.items.length > 0"
                  class="omni-select-all-tag"
                  :class="{ active: isAllSelected('left'), partial: isPartialSelected('left') }"
                >
                  <input
                    type="checkbox"
                    :checked="isAllSelected('left')"
                    :indeterminate="isPartialSelected('left')"
                    @change="toggleSelectAll('left')"
                  />
                  <span>全选 ({{ selectedCount('left') }}/{{ visibleItems('left').length }})</span>
                </label>

                <div class="omni-tags-list">
                  <div
                    v-for="tag in getExtTags('left')"
                    :key="tag.ext"
                    class="ext-tag"
                    :class="{ active: tag.selected === tag.total, partial: tag.selected > 0 && tag.selected < tag.total }"
                    @click="toggleExtSelect('left', tag.ext)"
                  >
                    {{ tag.ext }} ({{ tag.selected }}/{{ tag.total }})
                  </div>
                </div>
              </div>
            </div>
          </div>

          <!-- 面板 B (右) -->
          <div class="omni-panel omni-panel-right">
            <div class="omni-list-header">
              <b>面板 B</b>
              <div class="omni-tools">
                <button class="tool-btn" @click="renderPane('right', true)">🔄 刷新</button>
                <button class="tool-btn" @click="handleMkdir('right')">➕ 新建文件夹</button>
                <button class="tool-btn" @click="syncPath('right', 'left')">📍 同步给左侧</button>
              </div>
            </div>

            <div class="omni-path-container">
              <span class="omni-crumb" @click="navigateToPath('right', '/')">🏠 根目录</span>
              <template v-for="(crumb, idx) in getBreadcrumbs('right')" :key="crumb.fullPath">
                <span class="omni-crumb-sep">/</span>
                <span class="omni-crumb" @click="navigateToPath('right', crumb.fullPath)">{{ crumb.name }}</span>
              </template>
            </div>

            <div class="omni-list" :class="{ 'has-bottom-sheet': ctxMenu.visible && isMobile }">
              <div v-if="panes.right.loading" class="omni-empty">正在读取...</div>
              <div v-else-if="panes.right.error" class="omni-empty text-error">
                加载失败: {{ panes.right.error }}
              </div>
              <template v-else>
                <!-- 返回上一级 -->
                <div v-if="panes.right.path !== '/'" class="omni-item omni-item-up" @click="goUp('right')">
                  <span style="margin-right: 12px; width: 16px;"></span>
                  ⬅️ <b>返回上一级</b>
                </div>

                <!-- 文件列表 -->
                <div
                  v-for="item in visibleItems('right')"
                  :key="item.name"
                  class="omni-item data-row"
                  :class="{ 'omni-item-active': ctxMenu.visible && ctxMenu.side === 'right' && ctxMenu.item?.name === item.name }"
                  @click="handleItemClick('right', item, $event)"
                  @contextmenu.prevent="openContextMenu('right', item, $event)"
                >
                  <input
                    type="checkbox"
                    class="omni-cb"
                    :checked="isItemSelected('right', item.name)"
                    @click.stop="toggleItemSelect('right', item.name)"
                  />
                  <span class="omni-item-title">
                    {{ item.is_dir ? '📁 ' : '📄 ' }}{{ item.name }}
                  </span>
                  <span class="omni-more" @click.stop="openContextMenuMobile('right', item, $event)">⋯</span>
                </div>
              </template>
            </div>

            <!-- 状态筛选栏 -->
            <div class="omni-status-bar">
              <div class="omni-filter-row">
                <input
                  v-model="panes.right.filterText"
                  type="text"
                  class="omni-filter-input"
                  :placeholder="getFilterPlaceholder(panes.right.filterMode)"
                  @keydown.enter="applyFilter('right')"
                />
                <div class="omni-filter-modes">
                  <label><input v-model="panes.right.filterMode" type="radio" value="regex" /> 正则</label>
                  <label><input v-model="panes.right.filterMode" type="radio" value="keyword" /> 关键字</label>
                  <label><input v-model="panes.right.filterMode" type="radio" value="number" /> 选前N个</label>
                </div>
              </div>

              <div class="omni-tag-row">
                <label
                  v-if="panes.right.items.length > 0"
                  class="omni-select-all-tag"
                  :class="{ active: isAllSelected('right'), partial: isPartialSelected('right') }"
                >
                  <input
                    type="checkbox"
                    :checked="isAllSelected('right')"
                    :indeterminate="isPartialSelected('right')"
                    @change="toggleSelectAll('right')"
                  />
                  <span>全选 ({{ selectedCount('right') }}/{{ visibleItems('right').length }})</span>
                </label>

                <div class="omni-tags-list">
                  <div
                    v-for="tag in getExtTags('right')"
                    :key="tag.ext"
                    class="ext-tag"
                    :class="{ active: tag.selected === tag.total, partial: tag.selected > 0 && tag.selected < tag.total }"
                    @click="toggleExtSelect('right', tag.ext)"
                  >
                    {{ tag.ext }} ({{ tag.selected }}/{{ tag.total }})
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- 底部全局操作栏 -->
        <div class="omni-footer">
          <div>
            <button class="omni-btn btn-exit" @click="handleClose">退出并刷新</button>
          </div>

          <div v-if="mobileDstHint" class="omni-dst-hint">{{ mobileDstHint }}</div>

          <div class="omni-actions">
            <button class="omni-btn btn-purple" @click="openMovieRenameModal">
              <span class="bi">🎬</span><span class="bt">影视重命名</span>
            </button>
            <button class="omni-btn btn-warning" @click="openNativeRenameModal">
              <span class="bi">📝</span><span class="bt">批量重命名</span>
            </button>
            <button class="omni-btn btn-danger" :disabled="executingAction" @click="handleAction('remove')">
              <span class="bi">🗑️</span><span class="bt">删除</span>
            </button>
            <button class="omni-btn btn-success" :disabled="executingAction" @click="handleAction('copy')">
              <span class="bi">📄</span><span class="bt">复制</span>
            </button>
            <button class="omni-btn btn-primary" :disabled="executingAction" @click="handleAction('move')">
              <span class="bi">✈️</span><span class="bt">移动</span>
            </button>
          </div>
        </div>

        <!-- 右键 / 移动端底部抽屉上下文菜单 -->
        <div v-if="ctxMenu.visible" class="omni-menu-mask" @click="closeContextMenu"></div>
        <div
          v-if="ctxMenu.visible"
          class="omni-context-menu"
          :style="ctxMenuStyle"
        >
          <div class="omni-ctx-header">
            <div class="omni-ctx-info">
              <span>{{ ctxMenu.item?.is_dir ? '📁' : '📄' }}</span>
              <span class="omni-ctx-name">{{ ctxMenu.item?.name }}</span>
            </div>
            <button class="omni-ctx-close" @click="closeContextMenu">✕</button>
          </div>
          <div class="omni-ctx-items">
            <div v-if="!ctxMenu.item?.is_dir" class="omni-menu-item" @click="handleCtxMpv">▶️ MPV 播放</div>
            <div class="omni-menu-item" @click="handleCtxOpenInBrowser">🌐 浏览器打开</div>
            <div class="omni-menu-item" @click="handleCtxCopyUrl">🔗 复制 OpenList 链接</div>
            <div class="omni-menu-item" @click="handleCtxSingleRename">✏️ 重命名</div>
            <div class="omni-menu-item" @click="handleCtxTmdbScrape">🎬 TMDB 智能刮削</div>
            <div class="omni-menu-item" @click="handleCtxCopy">📄 复制到对侧</div>
            <div class="omni-menu-item" @click="handleCtxMove">✈️ 移动到对侧</div>
            <div class="omni-menu-item danger" @click="handleCtxDelete">🗑️ 删除</div>
          </div>
        </div>

        <!-- 模态框 1：单文件重命名 -->
        <div v-if="singleRename.visible" class="sub-modal-mask" @click.self="singleRename.visible = false">
          <div class="sub-modal-box single-modal-box">
            <h3 style="margin: 0 0 15px 0; font-size: 16px; color: #1e293b;">✏️ 重命名该项目</h3>
            <input
              ref="singleRenameInputRef"
              v-model="singleRename.name"
              type="text"
              class="rn-input"
              @keydown.enter="submitSingleRename"
              @keydown.esc="singleRename.visible = false"
            />
            <div style="display: flex; justify-content: flex-end; gap: 10px;">
              <button class="omni-btn btn-cancel" @click="singleRename.visible = false">取消</button>
              <button class="omni-btn btn-primary" :disabled="singleRename.submitting" @click="submitSingleRename">
                {{ singleRename.submitting ? '执行中...' : '确定' }}
              </button>
            </div>
          </div>
        </div>

        <!-- 模态框 2：批量重命名 (正则 / 顺序 / 查找替换) -->
        <div v-if="batchRename.visible" class="sub-modal-mask" @click.self="batchRename.visible = false">
          <div class="sub-modal-box batch-modal-box">
            <div class="rn-header">
              <span>批量重命名</span>
              <span class="rn-close-x" @click="batchRename.visible = false">✕</span>
            </div>

            <!-- 输入步骤 -->
            <div v-if="batchRename.step === 'input'" class="rn-body">
              <div class="rn-radio-group">
                <label class="rn-radio-label">
                  <input v-model="batchRename.mode" type="radio" value="regex" /> 正则表达式重命名
                </label>
                <label class="rn-radio-label">
                  <input v-model="batchRename.mode" type="radio" value="sequence" /> 顺序重命名
                </label>
                <label class="rn-radio-label">
                  <input v-model="batchRename.mode" type="radio" value="replace" /> 查找并替换
                </label>
              </div>

              <div class="rn-desc">
                <template v-if="batchRename.mode === 'regex'">
                  正则表达式文件重命名。在第一行输入源文件名正则表达式，并在第二行输入新文件名正则表达式。
                </template>
                <template v-else-if="batchRename.mode === 'sequence'">
                  新的文件名将会附加一个起始数值，并按顺序每次加 1。支持使用 <code>{number}</code> 占位符。
                </template>
                <template v-else>
                  查找并替换文件名中的指定字符串。
                </template>
              </div>

              <div v-if="batchRename.mode === 'regex'">
                <input v-model="batchRename.in1" type="text" class="rn-input" placeholder="源文件名正则表达式" />
                <input v-model="batchRename.in2" type="text" class="rn-input" placeholder="新文件名正则表达式" />
              </div>
              <div v-else-if="batchRename.mode === 'sequence'">
                <input v-model="batchRename.in1" type="text" class="rn-input" placeholder="新文件名模板 (如: 视频_{number})" />
                <input v-model="batchRename.in2" type="number" class="rn-input" placeholder="起始数值" />
                <input v-model="batchRename.in3" type="number" class="rn-input" placeholder="前导零填充宽度 (可选，如: 2)" />
              </div>
              <div v-else>
                <input v-model="batchRename.in1" type="text" class="rn-input" placeholder="查找内容" />
                <input v-model="batchRename.in2" type="text" class="rn-input" placeholder="替换为" />
              </div>

              <div style="display: flex; justify-content: flex-end; gap: 10px; margin-top: 20px;">
                <button class="omni-btn btn-cancel" @click="batchRename.visible = false">取消</button>
                <button class="omni-btn btn-primary" @click="generateBatchPreview">预览</button>
              </div>
            </div>

            <!-- 预览步骤 -->
            <div v-else class="rn-body rn-page-preview">
              <div style="font-size: 14px; font-weight: bold; margin-bottom: 10px; color: #334155;">更改后文件</div>
              <div class="rn-table-container">
                <table class="rn-table">
                  <thead>
                    <tr>
                      <th style="width: 50%;">旧名称</th>
                      <th style="width: 50%;">新名称</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr v-for="obj in batchRename.calculatedObjects" :key="obj.src_name">
                      <td style="color: #64748b;">{{ obj.src_name }}</td>
                      <td style="color: #0f172a; font-weight: bold;">{{ obj.new_name }}</td>
                    </tr>
                  </tbody>
                </table>
              </div>
              <div style="display: flex; justify-content: flex-end; gap: 10px;">
                <button class="omni-btn btn-cancel" @click="batchRename.visible = false">取消</button>
                <button class="omni-btn btn-cancel" @click="batchRename.step = 'input'">返回</button>
                <button class="omni-btn btn-primary" :disabled="batchRename.submitting" @click="submitBatchRename">
                  {{ batchRename.submitting ? '执行中...' : '确定' }}
                </button>
              </div>
            </div>
          </div>
        </div>

        <!-- 模态框 3：🎬 智能影视重命名面板 (带拖拽排序、单行铅笔精修、字幕后缀保留) -->
        <div v-if="movieRename.visible" class="sub-modal-mask">
          <div class="sub-modal-box movie-modal-box modal-box-flush">
            <div style="padding: 15px 20px; background: #f8fafc; border-bottom: 1px solid #e2e8f0; display: flex; justify-content: space-between; align-items: center;">
              <div>
                <h3 style="margin: 0; font-size: 16px; color: #1e293b; display: flex; align-items: center; gap: 8px;">
                  🎬 智能校验面板
                  <span style="background: #10b981; color: #fff; padding: 2px 6px; border-radius: 4px; font-size: 12px;">
                    {{ movieRename.paneRealSeason ? '剧集模式 (' + movieRename.paneRealSeason + ')' : '季度模式' }}
                  </span>
                </h3>
                <p style="margin: 4px 0 0 0; font-size: 12px; color: #64748b;">
                  目录: <span style="color: #334155; font-weight: 500;">{{ movieRename.srcDir }}</span>
                </p>
              </div>
              <span class="rn-close-x" @click="movieRename.visible = false">✕</span>
            </div>

            <!-- 设置折叠区 -->
            <details class="movie-settings-details" :open="!isMobile">
              <summary style="padding: 12px 20px; background: #f8fafc; font-weight: bold; cursor: pointer; user-select: none;">
                ⚙️ 展开/折叠 重命名设置选项
              </summary>
              <div style="padding: 10px 20px 15px; display: flex; flex-direction: column; gap: 15px;">
                <div>
                  <div class="modal-mode-wrapper" style="display: flex; gap: 20px; align-items: center; font-size: 13px; color: #334155;">
                    <label style="cursor: pointer; display: flex; align-items: center;">
                      <input v-model="movieRename.parseMode" type="radio" value="regex" />&nbsp;🧠 智能正则提取
                    </label>
                    <div style="display: flex; align-items: center; gap: 12px;">
                      <label style="cursor: pointer; display: flex; align-items: center;">
                        <input v-model="movieRename.parseMode" type="radio" value="sequential" />&nbsp;🔢 强制顺序递增
                      </label>
                      <div v-if="movieRename.parseMode === 'sequential'" style="display: flex; align-items: center; gap: 6px; font-size: 12px; color: #0369a1; background: #e0f2fe; padding: 4px 10px; border-radius: 6px; border: 1px solid #bae6fd;">
                        起始: <input v-model.number="movieRename.sequenceStartNum" type="number" min="0" style="width: 45px; padding: 2px 4px; border: 1px solid #7dd3fc; border-radius: 4px; text-align: center;" />
                      </div>
                    </div>
                    <div style="display: flex; align-items: center; gap: 6px; font-size: 12px; color: #475569; background: #f1f5f9; padding: 4px 10px; border-radius: 6px; border: 1px solid #cbd5e1; margin-left: auto;">
                      补零位: <input v-model.number="movieRename.sequencePadNum" type="number" min="1" max="5" style="width: 40px; padding: 2px 4px; border: 1px solid #cbd5e1; border-radius: 4px; text-align: center;" />
                    </div>
                  </div>
                </div>

                <div v-if="movieRename.paneRealSeason">
                  <div class="modal-mode-wrapper" style="display: flex; gap: 15px; align-items: center; font-size: 13px; color: #334155; flex-wrap: wrap;">
                    <label style="cursor: pointer;">
                      <input v-model="movieRename.prefixMode" type="radio" value="clean" /> 仅剧名
                      <span style="color: #10b981; font-weight: bold;">({{ movieRename.cleanMediaName || '未知' }})</span>
                    </label>
                    <label style="cursor: pointer;">
                      <input v-model="movieRename.prefixMode" type="radio" value="default" /> 纯净 (SxxExx)
                    </label>
                    <label style="cursor: pointer;">
                      <input v-model="movieRename.prefixMode" type="radio" value="folder" /> 附加目录名
                      <span style="color: #94a3b8; font-weight: bold;">({{ movieRename.autoFolderName || '未知' }})</span>
                    </label>
                    <label style="cursor: pointer;">
                      <input v-model="movieRename.prefixMode" type="radio" value="custom" /> 自定义名称
                    </label>
                  </div>
                  <div v-if="movieRename.prefixMode === 'custom'" style="margin-top: 12px;">
                    <input v-model="movieRename.customText" type="text" placeholder="输入要附加的前缀 (结尾加空格以分隔)" class="rn-input" style="margin-bottom: 0;" />
                  </div>
                </div>
                <div v-else style="background: #f1f5f9; padding: 8px 12px; border-radius: 6px; border: 1px dashed #cbd5e1; font-size: 12px; color: #64748b;">
                  🔒 季度模式强制生成标准格式 (如: Season 01)
                </div>

                <!-- 字幕设置 -->
                <div style="border-top: 1px dashed #e2e8f0; padding-top: 12px; margin-top: 5px;">
                  <div style="font-size: 13px; color: #334155; font-weight: bold; margin-bottom: 8px;">
                    📝 字幕语言后缀 (仅对 .srt/.ass/.vtt 等生效)
                  </div>
                  <div class="modal-mode-wrapper" style="display: flex; gap: 15px; align-items: center; font-size: 13px; color: #475569; flex-wrap: wrap;">
                    <label style="cursor: pointer;">
                      <input v-model="movieRename.subMode" type="radio" value="auto" /> 🤖 智能保留原标签
                    </label>
                    <div style="display: flex; align-items: center; gap: 5px;">
                      <label style="cursor: pointer;">
                        <input v-model="movieRename.subMode" type="radio" value="preset" /> 快捷预设:
                      </label>
                      <select v-model="movieRename.subPreset" style="padding: 4px; border-radius: 4px; border: 1px solid #cbd5e1; font-size: 12px;">
                        <option v-for="p in SUBTITLE_PRESETS" :key="p.value" :value="p.value">{{ p.label }}</option>
                      </select>
                    </div>
                    <div style="display: flex; align-items: center; gap: 5px;">
                      <label style="cursor: pointer;">
                        <input v-model="movieRename.subMode" type="radio" value="custom" /> 自定义:
                      </label>
                      <input v-model="movieRename.subCustom" type="text" placeholder="如: .简中.zh" style="width: 120px; padding: 4px 8px; border: 1px solid #cbd5e1; border-radius: 4px; font-size: 12px;" />
                    </div>
                    <label style="cursor: pointer;">
                      <input v-model="movieRename.subMode" type="radio" value="none" /> 不添加
                    </label>
                  </div>
                </div>
              </div>
            </details>

            <!-- 列表与告警栏 -->
            <div style="flex: 1; overflow-y: hidden; display: flex; flex-direction: column; position: relative;">
              <div v-if="movieComputedValidation.notice" style="padding: 12px 20px 0 20px;">
                <div :class="movieComputedValidation.noticeClass" style="padding: 12px; border-radius: 6px; font-weight: bold;">
                  {{ movieComputedValidation.notice }}
                </div>
              </div>

              <div class="modal-header-row" style="display: flex; padding: 10px 20px; background: #fff; font-weight: bold; font-size: 12px; color: #475569; align-items: center; border-bottom: 2px solid #e2e8f0;">
                <div style="width: 40px;"></div>
                <div style="width: 40px; text-align: center;">序号</div>
                <div style="flex: 1; padding-right: 15px;">原名称</div>
                <div style="width: 60px; text-align: center;">状态</div>
                <div style="flex: 1; padding-left: 15px;">实时预览</div>
                <button style="padding: 6px 12px; background: #64748b; color: #fff; border: none; border-radius: 4px; cursor: pointer; font-size: 12px; font-weight: bold;" @click="reverseMovieData">
                  ⇄ 反转
                </button>
              </div>

              <!-- 拖拽行列表 -->
              <div class="movie-rows-container">
                <div
                  v-for="(row, idx) in movieRename.items"
                  :key="row.src_name"
                  class="draggable-row"
                  :class="{ 'row-dup': rowStatus(idx).isDuplicate, 'row-err': rowStatus(idx).isMismatch, 'row-break': rowStatus(idx).isSeqBreak }"
                  draggable="true"
                  @dragstart="handleRowDragStart(idx, $event)"
                  @dragover.prevent="handleRowDragOver(idx, $event)"
                  @drop="handleRowDrop(idx, $event)"
                >
                  <div class="drag-handle">⋮⋮</div>
                  <div class="row-seq">[{{ idx + 1 }}]</div>
                  <div class="row-src">{{ row.src_name }}</div>
                  <div class="row-arrow">
                    <span v-if="rowStatus(idx).isDuplicate" class="badge-dup" title="检测到同名文件，下发会互相覆盖">🚨 重复</span>
                    <span v-else-if="rowStatus(idx).isMismatch" class="badge-err">❌ 不匹配</span>
                    <span v-else-if="rowStatus(idx).isSeqBreak" class="badge-warn" title="未按顺序递增或存在跳号">⚠️ 跳号</span>
                    <span v-else-if="row.manual_new_name" class="badge-custom">✍️ 自定</span>
                    <span v-else class="badge-ok">🟢 ➔</span>
                  </div>
                  <div class="row-new">
                    <!-- 行内铅笔编辑 -->
                    <template v-if="movieRename.editingIdx === idx">
                      <input
                        v-model="movieRename.editingName"
                        type="text"
                        class="inline-edit-input"
                        @keydown.enter="saveInlineEdit(idx)"
                        @keydown.esc="cancelInlineEdit"
                      />
                      <div class="inline-edit-actions">
                        <span class="inline-save-btn" title="保存确认" @click="saveInlineEdit(idx)">💾</span>
                        <span class="inline-cancel-btn" title="取消编辑" @click="cancelInlineEdit">✕</span>
                      </div>
                    </template>
                    <template v-else>
                      <span class="name-text-span">{{ getRowComputedName(idx) }}</span>
                      <div class="inline-actions">
                        <span class="inline-edit-btn" title="单行修改" @click="startInlineEdit(idx)">✏️</span>
                        <span v-if="row.manual_new_name" class="inline-reset-btn" title="恢复自动生成" @click="row.manual_new_name = null">↺</span>
                      </div>
                    </template>
                  </div>
                </div>
              </div>
            </div>

            <div style="padding: 15px 20px; background: #f8fafc; border-top: 1px solid #e2e8f0; display: flex; justify-content: flex-end; gap: 12px;">
              <button class="omni-btn btn-cancel" @click="movieRename.visible = false">取消</button>
              <button class="omni-btn btn-success" :disabled="movieRename.submitting" @click="submitMovieRename">
                {{ movieRename.submitting ? '执行中...' : '下发修改 ⚡' }}
              </button>
            </div>
          </div>
        </div>

        <!-- 模态框 4：🎬 TMDB 智能刮削面板 (含豆瓣、IMDb、TMDB 直达与 4 大格式) -->
        <div v-if="tmdbModal.visible" class="sub-modal-mask" @click.self="tmdbModal.visible = false">
          <div class="sub-modal-box tmdb-modal-box modal-box-flush">
            <div class="tmdb-ext-header">
              <div>
                <h3 style="margin: 0; font-size: 16px; color: #1e293b;">🎬 TMDB 智能刮削</h3>
                <p style="margin: 4px 0 0 0; font-size: 12px; color: #64748b;">
                  原文件: <b>{{ tmdbModal.srcName }}</b>
                </p>
              </div>
              <span class="rn-close-x" @click="tmdbModal.visible = false">✕</span>
            </div>

            <div class="tmdb-ext-body">
              <!-- 搜索与目标格式切换 -->
              <div class="tmdb-ext-search-area">
                <div class="tmdb-ext-search-row">
                  <input
                    v-model="tmdbModal.query"
                    type="text"
                    class="tmdb-ext-input"
                    placeholder="影视名称 (拼音或暗号请手动修改)"
                    @keydown.enter="performTmdbSearch"
                  />
                  <input
                    v-model="tmdbModal.year"
                    type="text"
                    class="tmdb-ext-input-year"
                    placeholder="年份 (可选)"
                    @keydown.enter="performTmdbSearch"
                  />
                  <button class="tmdb-ext-btn" :disabled="tmdbModal.searching" @click="performTmdbSearch">
                    {{ tmdbModal.searching ? '检索中...' : '🔍 搜索' }}
                  </button>
                </div>

                <div class="tmdb-ext-format-row">
                  <span style="color: #64748b;">目标格式:</span>
                  <label><input v-model="tmdbModal.format" type="radio" value="standard" /> 剧名 (年份) [tmdbid]</label>
                  <label><input v-model="tmdbModal.format" type="radio" value="full" /> 剧名 (年份) [tmdbid] [imdbid]</label>
                  <label><input v-model="tmdbModal.format" type="radio" value="year" /> 剧名 (年份)</label>
                  <label><input v-model="tmdbModal.format" type="radio" value="name" /> 仅剧名</label>
                </div>
              </div>

              <!-- 搜索结果列表 -->
              <div class="tmdb-ext-results-container">
                <div v-if="tmdbModal.searching" style="text-align: center; padding: 30px; color: #94a3b8;">
                  🔍 正在通过 TMDB 检索，并抓取 IMDb 数据...
                </div>
                <div v-else-if="tmdbModal.error" style="text-align: center; padding: 30px; color: #ef4444;">
                  ❌ {{ tmdbModal.error }}
                </div>
                <div v-else-if="tmdbModal.results.length === 0" style="text-align: center; padding: 30px; color: #94a3b8;">
                  等待搜索...
                </div>
                <div v-else>
                  <div
                    v-for="(card, index) in tmdbModal.results"
                    :key="card.id"
                    class="tmdb-ext-result-card"
                  >
                    <div class="tmdb-ext-card-content">
                      <img
                        :src="card.poster_path ? `https://image.tmdb.org/t/p/w200${card.poster_path}` : 'https://via.placeholder.com/60x90'"
                        class="tmdb-ext-poster"
                      />
                      <div class="tmdb-ext-info">
                        <h4 class="tmdb-ext-title">{{ card.displayTitle }}</h4>
                        <p class="tmdb-ext-meta">
                          {{ card.media_type === 'movie' ? '🎬 电影' : '📺 剧集' }} • {{ card.displayYear || '未知年份' }} • TMDB: {{ card.id }}
                          <template v-if="card.imdb_id"> • IMDb: {{ card.imdb_id }}</template>
                        </p>
                        <div class="tmdb-ext-preview-row">
                          ✨ 目标: <span class="tmdb-ext-preview-text">{{ getTmdbTargetName(card) }}</span>
                        </div>
                      </div>
                    </div>

                    <div class="tmdb-ext-actions">
                      <div class="tmdb-ext-link-group">
                        <a :href="card.tmdbUrl" target="_blank" rel="noopener noreferrer" class="tmdb-ext-link-btn tmdb-btn-tmdb">TMDB ↗</a>
                        <a :href="card.doubanUrl" target="_blank" rel="noopener noreferrer" class="tmdb-ext-link-btn tmdb-btn-douban">豆瓣 ↗</a>
                        <a :href="card.imdbUrl" target="_blank" rel="noopener noreferrer" class="tmdb-ext-link-btn tmdb-btn-imdb">IMDb ↗</a>
                      </div>
                      <button class="tmdb-ext-apply-btn" :disabled="card.applying" @click="applyTmdbRename(card)">
                        {{ card.applying ? '执行中...' : '⚡ 应用' }}
                      </button>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- 模态框 5：删除二次确认 (双栏内嵌模态框，还原 OpenList 原生风格) -->
        <div v-if="deleteModal.visible" class="sub-modal-mask" @click.self="deleteModal.visible = false">
          <div class="sub-modal-box delete-modal-box">
            <div class="rn-header" style="margin-bottom: 14px;">
              <span style="display: flex; align-items: center; gap: 6px; font-weight: bold; color: #1e293b;">
                🗑️ 确认删除
              </span>
              <span class="rn-close-x" @click="deleteModal.visible = false">✕</span>
            </div>
            <div style="font-size: 14px; color: #334155; margin-bottom: 12px; line-height: 1.6;">
              确定要删除选中的 <b style="color: #ef4444;">{{ deleteModal.names.length }}</b> 个项目吗？
            </div>
            <div
              v-if="deleteModal.names.length <= 6"
              style="max-height: 140px; overflow-y: auto; background: #f8fafc; border: 1px solid #e2e8f0; border-radius: 6px; padding: 8px 12px; margin-bottom: 16px; font-size: 12px; color: #64748b;"
            >
              <div v-for="name in deleteModal.names" :key="name" style="overflow: hidden; text-overflow: ellipsis; white-space: nowrap; line-height: 1.8;">
                • {{ name }}
              </div>
            </div>
            <div v-else style="font-size: 12px; color: #94a3b8; margin-bottom: 16px;">
              包含：{{ deleteModal.names.slice(0, 3).join('、') }} 等共 {{ deleteModal.names.length }} 项
            </div>
            <div style="display: flex; justify-content: flex-end; gap: 10px;">
              <button class="omni-btn btn-cancel" @click="deleteModal.visible = false">取消</button>
              <button class="omni-btn btn-danger" :disabled="deleteModal.submitting" @click="confirmDeleteAction">
                {{ deleteModal.submitting ? '删除中...' : '确定删除' }}
              </button>
            </div>
          </div>
        </div>

        <!-- 模态框 6：新建文件夹 (双栏内嵌模态框) -->
        <div v-if="mkdirModal.visible" class="sub-modal-mask" @click.self="mkdirModal.visible = false">
          <div class="sub-modal-box mkdir-modal-box">
            <div class="rn-header" style="margin-bottom: 14px;">
              <span style="display: flex; align-items: center; gap: 6px; font-weight: bold; color: #1e293b;">
                ➕ 新建文件夹
              </span>
              <span class="rn-close-x" @click="mkdirModal.visible = false">✕</span>
            </div>
            <div style="font-size: 13px; color: #64748b; margin-bottom: 10px;">
              在 <b>{{ panes[mkdirModal.side].path }}</b> 下新建文件夹：
            </div>
            <input
              ref="mkdirInputRef"
              v-model="mkdirModal.name"
              type="text"
              placeholder="请输入文件夹名称"
              class="rn-input"
              @keydown.enter="submitMkdir"
              @keydown.esc="mkdirModal.visible = false"
            />
            <div style="display: flex; justify-content: flex-end; gap: 10px; margin-top: 15px;">
              <button class="omni-btn btn-cancel" @click="mkdirModal.visible = false">取消</button>
              <button class="omni-btn btn-primary" :disabled="mkdirModal.submitting" @click="submitMkdir">
                {{ mkdirModal.submitting ? '创建中...' : '确定' }}
              </button>
            </div>
          </div>
        </div>

        <!-- 剧集连播悬浮控制栏 -->
        <EpisodePlayerBar
          :visible="!!activeEpisodePlayback"
          :filename="activeEpisodePlayback?.filename || ''"
          :all-filenames="activeEpisodePlayback ? (panes[activeEpisodePlayback.side].path === activeEpisodePlayback.dir ? panes[activeEpisodePlayback.side].files.map(f => f.name) : activeEpisodePlayback.cachedFilenames) : []"
          :play-mode="activeEpisodePlayback?.playMode || 'mpv'"
          @close="activeEpisodePlayback = null"
          @switch="handleEpisodeSwitch"
          @copy="handleEpisodeCopy"
          @re-launch="handleEpisodeReLaunch"
        />
      </div>
    </div>
  </teleport>
</template>

<script setup lang="ts">
import { ref, reactive, computed, watch, onMounted, nextTick } from 'vue'
import { ElMessage } from 'element-plus'
import { callOpenListFsBackend, proxyTmdbBackend } from '@/api/config'
import EpisodePlayerBar from '@/components/EpisodePlayerBar.vue'
import { useOpenListStore } from '@/stores/openlist'
import {
  launchMpv,
  openInBrowser,
  copyDirectUrl,
  isVideoFile,
  findNeighborEpisode,
  formatEpisodeLabel,
  splitPath,
} from '@/utils/openlist'

const openListStore = useOpenListStore()

// 剧集连播悬浮栏播放状态
const activeEpisodePlayback = ref<{
  side: 'left' | 'right'
  dir: string
  filename: string
  playMode: 'mpv' | 'browser'
  cachedFilenames: string[]
} | null>(null)

// ==========================================
// ⚙️ 用户预设与默认配置 (1:1 还原 aa.js)
// ==========================================
const TMDB_DEFAULT_TOKEN = '81e0ba6435d7fe68dd96904cd3593642'
const SUBTITLE_PRESETS = [
  { label: '.简中.zh', value: '.简中.zh' },
  { label: '.繁中.zh', value: '.繁中.zh' },
  { label: '.en', value: '.en' },
  { label: '.简中.zh-CN', value: '.简中.zh-CN' },
  { label: '.繁中.zh-TW', value: '.繁中.zh-TW' },
  { label: '无标签 (清空)', value: '_blank' },
]

// ==========================================
// 方案 A：本地存储持久化偏好设置 (自动记忆上次选择)
// ==========================================
const STORAGE_KEY_MOVIE_PREFS = 'pcs_dual_movie_rename_prefs'
const STORAGE_KEY_TMDB_PREFS = 'pcs_dual_tmdb_prefs'

interface MovieRenamePrefs {
  prefixMode?: 'clean' | 'default' | 'folder' | 'custom'
  customText?: string
  subMode?: 'auto' | 'preset' | 'custom' | 'none'
  subPreset?: string
  subCustom?: string
}

function loadMovieRenamePrefs(): MovieRenamePrefs {
  try {
    const raw = localStorage.getItem(STORAGE_KEY_MOVIE_PREFS)
    return raw ? JSON.parse(raw) : {}
  } catch {
    return {}
  }
}

function saveMovieRenamePrefs() {
  try {
    localStorage.setItem(
      STORAGE_KEY_MOVIE_PREFS,
      JSON.stringify({
        prefixMode: movieRename.prefixMode,
        customText: movieRename.customText,
        subMode: movieRename.subMode,
        subPreset: movieRename.subPreset,
        subCustom: movieRename.subCustom,
      })
    )
  } catch {}
}

interface TmdbPrefs {
  format?: 'standard' | 'full' | 'year' | 'name'
}

function loadTmdbPrefs(): TmdbPrefs {
  try {
    const raw = localStorage.getItem(STORAGE_KEY_TMDB_PREFS)
    return raw ? JSON.parse(raw) : {}
  } catch {
    return {}
  }
}

function saveTmdbPrefs() {
  try {
    localStorage.setItem(
      STORAGE_KEY_TMDB_PREFS,
      JSON.stringify({
        format: tmdbModal.format,
      })
    )
  } catch {}
}

// 正则表达式组
const REGEX = {
  SEASON: /S(E)?\d+/i,
  MULTI_EP: /E(\d+)[-–_]E?(\d+)/i,
  MULTI_NUM: /[\.\-_](\d+)[\.\-_](\d+)[\.\-_]*$/,
  SINGLE_EP: /(?:E|Ep|Episode|_)(\d+)/i,
  CLEANUP: /(1080|720|2160|4k|8k)[pi]?|[hx]26[45]|dts(-hd)?|truehd|atmos|aac|mp3|ac3|flac|5\.1|7\.1|10bit|8bit|remux|bluray|bdrip|webrip|web-dl|hdr|dolby|vision/gi,
  NUM_EXTRACT: /\d+/g,
  YEAR_TAG: /\s*[\(\[]\d{4}[\)\]]\s*/g,
  SEASON_FOLDER: /^(?:S|Season\s*)(\d+)$/i,
  SUB_EXT: /\.(srt|ass|ssa|vtt|sub|idx)$/i,
  SUB_LANG: /[\.\-_](zh[\-\_]?(cn|tw|hans|hant)|chs|cht|eng|jpn|kor|chi|tc|sc|简中?|繁中?|中英|中|英|双语|特效|gb|big5)[a-zA-Z0-9\-\&]*$/i,
}

interface OpenListItem {
  name: string
  size?: number
  is_dir: boolean
  modified?: string
  sign?: string
  thumb?: string
  type?: number
}

interface PaneState {
  path: string
  items: OpenListItem[]
  selectedNames: Set<string>
  filterText: string
  filterMode: 'regex' | 'keyword' | 'number'
  loading: boolean
  error: string | null
}

const props = defineProps<{
  modelValue: boolean
  initialPath?: string
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', val: boolean): void
  (e: 'close'): void
}>()

const visible = computed({
  get: () => props.modelValue,
  set: (val) => emit('update:modelValue', val),
})

const isMobile = ref(window.innerWidth < 768)
const activeMobileTab = ref<'left' | 'right'>('left')
const executingAction = ref(false)

const panes = reactive<{ left: PaneState; right: PaneState }>({
  left: {
    path: '/',
    items: [],
    selectedNames: new Set(),
    filterText: '',
    filterMode: 'regex',
    loading: false,
    error: null,
  },
  right: {
    path: '/',
    items: [],
    selectedNames: new Set(),
    filterText: '',
    filterMode: 'regex',
    loading: false,
    error: null,
  },
})

// 监听打开弹窗
watch(
  () => props.modelValue,
  (val) => {
    if (val) {
      const initP = props.initialPath || '/'
      panes.left.path = initP
      panes.right.path = initP
      renderPane('left')
      renderPane('right')
    }
  }
)

function handleClose() {
  visible.value = false
  emit('close')
}

function getPaneTabName(side: 'left' | 'right'): string {
  const p = panes[side].path
  return p.split('/').filter(Boolean).pop() || '根目录'
}

function selectedCount(side: 'left' | 'right'): number {
  return panes[side].selectedNames.size
}

const mobileDstHint = computed(() => {
  const l = panes.left.selectedNames.size
  const r = panes.right.selectedNames.size
  if (l > 0 && r === 0) return `A → B：${panes.right.path}`
  if (r > 0 && l === 0) return `B → A：${panes.left.path}`
  return ''
})

function getBreadcrumbs(side: 'left' | 'right') {
  const parts = panes[side].path.split('/').filter(Boolean)
  let curr = ''
  return parts.map((p) => {
    curr += '/' + p
    return { name: p, fullPath: curr }
  })
}

function navigateToPath(side: 'left' | 'right', p: string) {
  panes[side].path = p || '/'
  renderPane(side)
}

function goUp(side: 'left' | 'right') {
  const parts = panes[side].path.split('/').filter(Boolean)
  parts.pop()
  panes[side].path = parts.length === 0 ? '/' : '/' + parts.join('/')
  renderPane(side)
}

function syncPath(from: 'left' | 'right', to: 'left' | 'right') {
  panes[to].path = panes[from].path
  renderPane(to)
}

function getFilterPlaceholder(mode: 'regex' | 'keyword' | 'number') {
  return mode === 'regex' ? '正则筛选 (回车)' : mode === 'keyword' ? '关键字筛选 (回车)' : '输入数字选前N个 (回车)'
}

async function renderPane(side: 'left' | 'right', forceRefresh = false) {
  panes[side].loading = true
  panes[side].error = null
  panes[side].selectedNames.clear()

  try {
    const res = await callOpenListFsBackend<any>(
      '/api/fs/list',
      {
        path: panes[side].path,
        password: '',
        page: 1,
        per_page: 0,
        refresh: forceRefresh,
      }
    )
    let rawList: any[] = []
    if (Array.isArray(res)) {
      rawList = res
    } else if (Array.isArray(res?.data?.content)) {
      rawList = res.data.content
    } else if (Array.isArray(res?.content)) {
      rawList = res.content
    } else if (Array.isArray(res?.data)) {
      rawList = res.data
    }
    const list: OpenListItem[] = [...rawList]
    list.sort((a, b) =>
      a.is_dir !== b.is_dir
        ? b.is_dir
          ? 1
          : -1
        : a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: 'base' })
    )
    panes[side].items = list
  } catch (err: any) {
    panes[side].error = err.message || '加载目录失败'
  } finally {
    panes[side].loading = false
  }
}

function visibleItems(side: 'left' | 'right'): OpenListItem[] {
  return panes[side].items
}

function isItemSelected(side: 'left' | 'right', name: string): boolean {
  return panes[side].selectedNames.has(name)
}

function toggleItemSelect(side: 'left' | 'right', name: string) {
  if (panes[side].selectedNames.has(name)) {
    panes[side].selectedNames.delete(name)
  } else {
    panes[side].selectedNames.add(name)
  }
}

function handleItemClick(side: 'left' | 'right', item: OpenListItem, e: MouseEvent) {
  if (item.is_dir) {
    panes[side].path = panes[side].path === '/' ? '/' + item.name : `${panes[side].path}/${item.name}`
    renderPane(side)
  } else {
    toggleItemSelect(side, item.name)
  }
}

function isAllSelected(side: 'left' | 'right'): boolean {
  const vis = visibleItems(side)
  return vis.length > 0 && vis.every((i) => panes[side].selectedNames.has(i.name))
}

function isPartialSelected(side: 'left' | 'right'): boolean {
  const count = panes[side].selectedNames.size
  const total = visibleItems(side).length
  return count > 0 && count < total
}

function toggleSelectAll(side: 'left' | 'right') {
  const allSel = isAllSelected(side)
  if (allSel) {
    panes[side].selectedNames.clear()
  } else {
    visibleItems(side).forEach((i) => panes[side].selectedNames.add(i.name))
  }
}

function applyFilter(side: 'left' | 'right') {
  const val = panes[side].filterText.trim()
  const mode = panes[side].filterMode
  if (mode === 'number') {
    if (/^\d+$/.test(val)) {
      const count = parseInt(val, 10)
      panes[side].selectedNames.clear()
      panes[side].items.slice(0, count).forEach((it) => panes[side].selectedNames.add(it.name))
    } else if (val) {
      ElMessage.warning('选前N个模式下，请输入纯数字！')
    }
    return
  }

  let regex: RegExp | null = null
  if (mode === 'regex' && val) {
    try {
      regex = new RegExp(val, 'i')
    } catch {
      regex = null
    }
  }

  panes[side].selectedNames.clear()
  panes[side].items.forEach((it) => {
    const isMatch = mode === 'regex' ? (regex ? regex.test(it.name) : false) : it.name.toLowerCase().includes(val.toLowerCase())
    if (isMatch) panes[side].selectedNames.add(it.name)
  })
}

function getExtTags(side: 'left' | 'right') {
  const map: Record<string, { total: number; selected: number }> = {}
  panes[side].items.forEach((it) => {
    if (!it.is_dir) {
      const dot = it.name.lastIndexOf('.')
      const ext = dot > 0 ? it.name.substring(dot).toLowerCase() : '无后缀'
      if (!map[ext]) map[ext] = { total: 0, selected: 0 }
      map[ext].total++
      if (panes[side].selectedNames.has(it.name)) {
        map[ext].selected++
      }
    }
  })
  return Object.keys(map)
    .sort()
    .map((ext) => ({ ext, total: map[ext].total, selected: map[ext].selected }))
}

function toggleExtSelect(side: 'left' | 'right', ext: string) {
  const extFiles = panes[side].items.filter((it) => {
    if (it.is_dir) return false
    const dot = it.name.lastIndexOf('.')
    const itemExt = dot > 0 ? it.name.substring(dot).toLowerCase() : '无后缀'
    return itemExt === ext
  })
  const allChecked = extFiles.every((it) => panes[side].selectedNames.has(it.name))
  extFiles.forEach((it) => {
    if (allChecked) panes[side].selectedNames.delete(it.name)
    else panes[side].selectedNames.add(it.name)
  })
}

// ==========================================
// ➕ 新建文件夹 (内嵌模态框)
// ==========================================
const mkdirModal = reactive({
  visible: false,
  side: 'left' as 'left' | 'right',
  name: '',
  submitting: false,
})
const mkdirInputRef = ref<HTMLInputElement | null>(null)

function handleMkdir(side: 'left' | 'right') {
  mkdirModal.side = side
  mkdirModal.name = ''
  mkdirModal.submitting = false
  mkdirModal.visible = true
  nextTick(() => {
    mkdirInputRef.value?.focus()
  })
}

async function submitMkdir() {
  const name = mkdirModal.name.trim()
  if (!name) {
    ElMessage.warning('文件夹名称不能为空')
    return
  }
  mkdirModal.submitting = true
  const side = mkdirModal.side
  try {
    const parent = panes[side].path
    const newPath = parent === '/' ? `/${name}` : `${parent}/${name}`
    await callOpenListFsBackend('/api/fs/mkdir', { path: newPath }, parent)
    ElMessage.success('新建文件夹成功')
    mkdirModal.visible = false
    renderPane(side, true)
  } catch (err: any) {
    ElMessage.error(err.message || '新建文件夹失败')
  } finally {
    mkdirModal.submitting = false
  }
}

// ==========================================
// 上下文菜单与文件操作 / 媒体播放
// ==========================================
const ctxMenu = reactive({
  visible: false,
  side: 'left' as 'left' | 'right',
  item: null as OpenListItem | null,
  x: 0,
  y: 0,
})

const ctxMenuStyle = computed(() => {
  if (isMobile.value) return {}
  const x = Math.min(ctxMenu.x, window.innerWidth - 220)
  const y = Math.min(ctxMenu.y, window.innerHeight - 380)
  return { left: `${Math.max(10, x)}px`, top: `${Math.max(10, y)}px` }
})

function getOpenListItemUrls(side: 'left' | 'right', item: OpenListItem) {
  const base = (openListStore.config.baseUrl || '').replace(/\/+$/, '')
  const parent = panes[side].path
  const fullPath = parent === '/' ? `/${item.name}` : `${parent}/${item.name}`
  const encodedParts = splitPath(fullPath).map(encodeURIComponent).join('/')
  const directUrl = `${base}/d/${encodedParts}`
  const browseUrl = `${base}/${encodedParts}`
  return {
    directUrl,
    browseUrl,
    fullPath,
    parent,
  }
}

function handleCtxMpv() {
  if (!ctxMenu.item) return
  const item = ctxMenu.item
  const side = ctxMenu.side
  closeContextMenu()
  const { directUrl } = getOpenListItemUrls(side, item)
  launchMpv(directUrl, item.size, item.name, openListStore.config.desktopScheme)
  if (isVideoFile(item.name)) {
    activeEpisodePlayback.value = {
      side,
      dir: panes[side].path,
      filename: item.name,
      playMode: 'mpv',
      cachedFilenames: panes[side].items.map((f) => f.name),
    }
  }
}

function handleCtxOpenInBrowser() {
  if (!ctxMenu.item) return
  const item = ctxMenu.item
  const side = ctxMenu.side
  closeContextMenu()
  const { directUrl, browseUrl } = getOpenListItemUrls(side, item)
  if (item.is_dir) {
    window.open(browseUrl, '_blank')
  } else {
    openInBrowser(directUrl, item.size, item.name)
    if (isVideoFile(item.name)) {
      activeEpisodePlayback.value = {
        side,
        dir: panes[side].path,
        filename: item.name,
        playMode: 'browser',
        cachedFilenames: panes[side].items.map((f) => f.name),
      }
    }
  }
}

function handleCtxCopyUrl() {
  if (!ctxMenu.item) return
  const item = ctxMenu.item
  const side = ctxMenu.side
  closeContextMenu()
  const { directUrl, browseUrl } = getOpenListItemUrls(side, item)
  if (item.is_dir) {
    void copyDirectUrl(browseUrl, 'OpenList 文件夹链接已复制')
  } else {
    void copyDirectUrl(directUrl, 'OpenList 直链已复制')
  }
}

function handleEpisodeSwitch(dir: -1 | 1) {
  if (!activeEpisodePlayback.value) return
  const currentPb = activeEpisodePlayback.value
  const side = currentPb.side
  const allNames =
    panes[side].path === currentPb.dir
      ? panes[side].items.map((f) => f.name)
      : currentPb.cachedFilenames

  const neighbor = findNeighborEpisode(allNames, currentPb.filename, dir)
  if (neighbor.error) {
    ElMessage.warning(neighbor.error)
    return
  }

  if (neighbor.name) {
    const targetItem = panes[side].items.find((f) => f.name === neighbor.name)
    const base = (openListStore.config.baseUrl || '').replace(/\/+$/, '')
    const fullPath = currentPb.dir === '/' ? `/${neighbor.name}` : `${currentPb.dir}/${neighbor.name}`
    const encoded = splitPath(fullPath).map(encodeURIComponent).join('/')
    const directUrl = `${base}/d/${encoded}`
    const label = (neighbor.key ? formatEpisodeLabel(neighbor.key) + '：' : '') + neighbor.name

    if (currentPb.playMode === 'mpv') {
      launchMpv(directUrl, targetItem?.size, label, openListStore.config.desktopScheme)
    } else {
      openInBrowser(directUrl, targetItem?.size, label)
    }
    activeEpisodePlayback.value.filename = neighbor.name
    ElMessage.success(`已切换至${dir > 0 ? '下一集' : '上一集'}：${label}`)
  }
}

function handleEpisodeCopy() {
  if (!activeEpisodePlayback.value) return
  const currentPb = activeEpisodePlayback.value
  const base = (openListStore.config.baseUrl || '').replace(/\/+$/, '')
  const fullPath = currentPb.dir === '/' ? `/${currentPb.filename}` : `${currentPb.dir}/${currentPb.filename}`
  const encoded = splitPath(fullPath).map(encodeURIComponent).join('/')
  void copyDirectUrl(`${base}/d/${encoded}`)
}

function handleEpisodeReLaunch() {
  if (!activeEpisodePlayback.value) return
  const currentPb = activeEpisodePlayback.value
  const side = currentPb.side
  const targetItem = panes[side].items.find((f) => f.name === currentPb.filename)
  const base = (openListStore.config.baseUrl || '').replace(/\/+$/, '')
  const fullPath = currentPb.dir === '/' ? `/${currentPb.filename}` : `${currentPb.dir}/${currentPb.filename}`
  const encoded = splitPath(fullPath).map(encodeURIComponent).join('/')
  const directUrl = `${base}/d/${encoded}`
  const label = currentPb.filename

  if (currentPb.playMode === 'mpv') {
    launchMpv(directUrl, targetItem?.size, label, openListStore.config.desktopScheme)
  } else {
    openInBrowser(directUrl, targetItem?.size, label)
  }
}

function openContextMenu(side: 'left' | 'right', item: OpenListItem, e: MouseEvent) {
  ctxMenu.visible = true
  ctxMenu.side = side
  ctxMenu.item = item
  ctxMenu.x = e.clientX
  ctxMenu.y = e.clientY
  panes[side].selectedNames.clear()
  panes[side].selectedNames.add(item.name)
}

function openContextMenuMobile(side: 'left' | 'right', item: OpenListItem, e: MouseEvent) {
  ctxMenu.visible = true
  ctxMenu.side = side
  ctxMenu.item = item
  panes[side].selectedNames.clear()
  panes[side].selectedNames.add(item.name)
}

function closeContextMenu() {
  ctxMenu.visible = false
}

// 单文件重命名
const singleRename = reactive({
  visible: false,
  side: 'left' as 'left' | 'right',
  oldName: '',
  name: '',
  submitting: false,
})
const singleRenameInputRef = ref<HTMLInputElement | null>(null)

function handleCtxSingleRename() {
  if (!ctxMenu.item) return
  singleRename.side = ctxMenu.side
  singleRename.oldName = ctxMenu.item.name
  singleRename.name = ctxMenu.item.name
  singleRename.visible = true
  closeContextMenu()
  nextTick(() => {
    if (singleRenameInputRef.value) {
      singleRenameInputRef.value.focus()
      const dot = singleRename.name.lastIndexOf('.')
      if (dot > 0) singleRenameInputRef.value.setSelectionRange(0, dot)
      else singleRenameInputRef.value.select()
    }
  })
}

async function submitSingleRename() {
  const newName = singleRename.name.trim()
  if (!newName || newName === singleRename.oldName) {
    singleRename.visible = false
    return
  }
  singleRename.submitting = true
  try {
    const parent = panes[singleRename.side].path
    const fullPath = parent === '/' ? `/${singleRename.oldName}` : `${parent}/${singleRename.oldName}`
    await callOpenListFsBackend('/api/fs/rename', { path: fullPath, name: newName }, parent)
    ElMessage.success('重命名成功')
    singleRename.visible = false
    renderPane(singleRename.side, true)
  } catch (err: any) {
    ElMessage.error(err.message || '重命名失败')
  } finally {
    singleRename.submitting = false
  }
}

function handleCtxDelete() {
  closeContextMenu()
  handleAction('remove')
}

function handleCtxCopy() {
  closeContextMenu()
  handleAction('copy')
}

function handleCtxMove() {
  closeContextMenu()
  handleAction('move')
}

// ==========================================
// 底部全局操作：复制 / 移动 / 删除
// ==========================================
function getSelectedContext() {
  const l = Array.from(panes.left.selectedNames)
  const r = Array.from(panes.right.selectedNames)
  if (l.length > 0 && r.length > 0) {
    ElMessage.warning('⚠️ 请仅在一侧勾选文件进行操作')
    return null
  }
  if (l.length === 0 && r.length === 0) {
    ElMessage.warning('⚠️ 请至少勾选一个项目！')
    return null
  }
  const fromSide: 'left' | 'right' = l.length > 0 ? 'left' : 'right'
  const toSide: 'left' | 'right' = fromSide === 'left' ? 'right' : 'left'
  return {
    side: fromSide,
    src_dir: panes[fromSide].path,
    dst_dir: panes[toSide].path,
    names: fromSide === 'left' ? l : r,
  }
}

// 模态框 5：删除二次确认状态
const deleteModal = reactive({
  visible: false,
  dir: '',
  names: [] as string[],
  submitting: false,
})

async function confirmDeleteAction() {
  if (!deleteModal.dir || deleteModal.names.length === 0) return
  deleteModal.submitting = true
  try {
    await callOpenListFsBackend('/api/fs/remove', { dir: deleteModal.dir, names: deleteModal.names }, deleteModal.dir)
    ElMessage.success(`✅ 成功删除了 ${deleteModal.names.length} 个项目`)
    deleteModal.visible = false
    renderPane('left', true)
    renderPane('right', true)
  } catch (err: any) {
    ElMessage.error(err.message || '删除失败')
  } finally {
    deleteModal.submitting = false
  }
}

async function handleAction(actionType: 'copy' | 'move' | 'remove') {
  const ctx = getSelectedContext()
  if (!ctx) return

  if (actionType === 'remove') {
    deleteModal.dir = ctx.src_dir
    deleteModal.names = [...ctx.names]
    deleteModal.visible = true
    return
  }

  if (ctx.dst_dir === ctx.src_dir) {
    ElMessage.warning('⚠️ 源路径与目标路径相同，无需操作！')
    return
  }

  executingAction.value = true
  const opName = actionType === 'move' ? '移动' : '复制'
  try {
    const payload = {
      src_dir: ctx.src_dir,
      dst_dir: ctx.dst_dir,
      names: ctx.names,
      merge: false,
      overwrite: false,
      skip_existing: false,
    }
    await callOpenListFsBackend(`/api/fs/${actionType}`, payload, ctx.src_dir)
    ElMessage.success(`✅ 成功${opName}了 ${ctx.names.length} 个文件！`)
    setTimeout(() => {
      renderPane('left', true)
      renderPane('right', true)
    }, 600)
  } catch (err: any) {
    ElMessage.error(err.message || `${opName}失败`)
  } finally {
    executingAction.value = false
  }
}

// ==========================================
// 模态框 2：批量重命名 (正则 / 顺序 / 查找替换)
// ==========================================
const batchRename = reactive({
  visible: false,
  step: 'input' as 'input' | 'preview',
  mode: 'regex' as 'regex' | 'sequence' | 'replace',
  in1: '',
  in2: '',
  in3: '',
  submitting: false,
  context: null as ReturnType<typeof getSelectedContext>,
  calculatedObjects: [] as Array<{ src_name: string; new_name: string }>,
})

function openNativeRenameModal() {
  const ctx = getSelectedContext()
  if (!ctx) return
  batchRename.context = ctx
  batchRename.step = 'input'
  batchRename.mode = 'regex'
  batchRename.in1 = ''
  batchRename.in2 = ''
  batchRename.in3 = ''
  batchRename.visible = true
}

function generateBatchPreview() {
  if (!batchRename.context) return
  const names = batchRename.context.names
  batchRename.calculatedObjects = names.map((oldName, index) => {
    let newName = oldName
    try {
      if (batchRename.mode === 'regex') {
        newName = oldName.replace(new RegExp(batchRename.in1, 'g'), batchRename.in2)
      } else if (batchRename.mode === 'replace') {
        newName = oldName.split(batchRename.in1).join(batchRename.in2)
      } else if (batchRename.mode === 'sequence') {
        const start = parseInt(batchRename.in2, 10) || 1
        const pad = parseInt(batchRename.in3, 10) || 1
        const numStr = String(start + index).padStart(pad, '0')
        const itemMeta = panes[batchRename.context!.side].items.find((i) => i.name === oldName)
        const dot = oldName.lastIndexOf('.')
        const ext = itemMeta && itemMeta.is_dir ? '' : dot > 0 ? oldName.substring(dot) : ''
        const baseNewName = batchRename.in1.includes('{number}')
          ? batchRename.in1.replace(/\{number\}/g, numStr)
          : batchRename.in1 + numStr
        newName = baseNewName + ext
      }
    } catch {}
    return { src_name: oldName, new_name: newName }
  })
  batchRename.step = 'preview'
}

async function submitBatchRename() {
  if (!batchRename.context) return
  batchRename.submitting = true
  try {
    await callOpenListFsBackend(
      '/api/fs/batch_rename',
      {
        src_dir: batchRename.context.src_dir,
        rename_objects: batchRename.calculatedObjects,
      },
      batchRename.context.src_dir
    )
    ElMessage.success(`批量重命名成功 (${batchRename.calculatedObjects.length}个)`)
    batchRename.visible = false
    renderPane(batchRename.context.side, true)
  } catch (err: any) {
    ElMessage.error(err.message || '重命名失败')
  } finally {
    batchRename.submitting = false
  }
}

// ==========================================
// 模态框 3：🎬 智能影视重命名引擎
// ==========================================
function extractSeasonFromPath(dirPath: string): string | null {
  const segments = dirPath.split('/').filter(Boolean)
  for (let i = segments.length - 1; i >= 0; i--) {
    const m = segments[i].match(REGEX.SEASON_FOLDER)
    if (m) return 'S' + m[1].padStart(2, '0')
  }
  return null
}

function extractCleanMediaName(rawName: string): string {
  if (!rawName) return ''
  let name = rawName
  name = name.replace(/\[\s*(?:tmdb(?:id)?|imdb(?:id)?)[^\]]*\]/gi, '')
  name = name.replace(/\(\s*(?:tmdb(?:id)?|imdb(?:id)?)[^\)]*\)/gi, '')
  name = name.replace(/[\(\[]\s*(?:19|20)\d{2}\s*[\)\]]/g, '')
  name = name.replace(/\b(?:19|20)\d{2}\b/g, '')
  name = name.replace(REGEX.CLEANUP, '')
  name = name.replace(/^[\.\-\_\s]+|[\.\-\_\s]+$/g, '').trim()
  return name
}

function analyzeEpisodeNumbers(filename: string, padWidth = 2) {
  if (/^S\d+$/i.test(filename)) {
    const sNum = parseInt(filename.replace(/S/i, ''), 10)
    return { first: sNum, numStr: String(sNum).padStart(padWidth, '0'), extra: '' }
  }
  const nameWithoutExt = filename.substring(0, filename.lastIndexOf('.')) || filename
  const withoutSeason = nameWithoutExt.replace(REGEX.SEASON, '')

  const multiEP_Match = withoutSeason.match(REGEX.MULTI_EP)
  if (multiEP_Match) {
    return {
      first: parseInt(multiEP_Match[1], 10),
      numStr: String(parseInt(multiEP_Match[1], 10)).padStart(padWidth, '0'),
      extra: `-E${String(parseInt(multiEP_Match[2], 10)).padStart(padWidth, '0')}`,
    }
  }
  const multiNum_Match = withoutSeason.match(REGEX.MULTI_NUM)
  if (multiNum_Match) {
    const f = parseInt(multiNum_Match[1], 10),
      s = parseInt(multiNum_Match[2], 10)
    if (s === f + 1) return { first: f, numStr: String(f).padStart(padWidth, '0'), extra: `-E${String(s).padStart(padWidth, '0')}` }
  }
  const singleEP_Match = withoutSeason.match(REGEX.SINGLE_EP)
  if (singleEP_Match) {
    return { first: parseInt(singleEP_Match[1], 10), numStr: String(parseInt(singleEP_Match[1], 10)).padStart(padWidth, '0'), extra: '' }
  }

  const cleaned = withoutSeason.replace(REGEX.CLEANUP, '')
  const numbers = cleaned.match(REGEX.NUM_EXTRACT)
  if (numbers && numbers.length > 0) {
    const lastNum = parseInt(numbers[numbers.length - 1], 10)
    return { first: lastNum, numStr: String(lastNum).padStart(padWidth, '0'), extra: '' }
  }
  return { first: null, numStr: String(1).padStart(padWidth, '0'), extra: '' }
}

function analyzeSeasonNumber(filename: string, padWidth = 2) {
  const match = filename.match(/(?:S|Season\s*)(\d+)/i)
  if (match) return { first: parseInt(match[1], 10), numStr: String(parseInt(match[1], 10)).padStart(padWidth, '0') }
  const numbers = filename.match(REGEX.NUM_EXTRACT)
  if (numbers && numbers.length > 0) {
    const num = parseInt(numbers[numbers.length - 1], 10)
    return { first: num, numStr: String(num).padStart(padWidth, '0') }
  }
  return { first: null, numStr: String(1).padStart(padWidth, '0') }
}

interface MovieRenameRow {
  src_name: string
  is_dir: boolean
  manual_new_name: string | null
}

const movieRename = reactive({
  visible: false,
  srcDir: '',
  paneRealSeason: null as string | null,
  cleanMediaName: '',
  autoFolderName: '',
  items: [] as MovieRenameRow[],
  parseMode: 'regex' as 'regex' | 'sequential',
  prefixMode: 'clean' as 'clean' | 'default' | 'folder' | 'custom',
  customText: '',
  sequenceStartNum: 1,
  sequencePadNum: 2,
  subMode: 'auto' as 'auto' | 'preset' | 'custom' | 'none',
  subPreset: '.简中.zh',
  subCustom: '',
  editingIdx: null as number | null,
  editingName: '',
  submitting: false,
  side: 'left' as 'left' | 'right',
})

function openMovieRenameModal() {
  const ctx = getSelectedContext()
  if (!ctx) return
  movieRename.side = ctx.side
  movieRename.srcDir = ctx.src_dir

  const segments = ctx.src_dir.split('/').filter(Boolean)
  let rawFolderName = ''
  if (segments.length >= 1) {
    const lastSegment = decodeURIComponent(segments[segments.length - 1])
    rawFolderName = REGEX.SEASON_FOLDER.test(lastSegment) && segments.length >= 2 ? decodeURIComponent(segments[segments.length - 2]) : lastSegment
  }
  movieRename.autoFolderName = rawFolderName.replace(REGEX.YEAR_TAG, '').trim()
  movieRename.cleanMediaName = extractCleanMediaName(rawFolderName)
  movieRename.paneRealSeason = extractSeasonFromPath(ctx.src_dir)

  const sideItems = panes[ctx.side].items
  const rows: MovieRenameRow[] = ctx.names.map((name) => {
    const meta = sideItems.find((i) => i.name === name)
    return { src_name: name, is_dir: meta ? meta.is_dir : false, manual_new_name: null }
  })
  movieRename.items = rows

  // 加载记忆偏好 (方案 A)
  const savedMoviePrefs = loadMovieRenamePrefs()
  movieRename.parseMode = 'regex'
  movieRename.prefixMode = savedMoviePrefs.prefixMode ?? (movieRename.paneRealSeason ? 'clean' : 'default')
  movieRename.customText = savedMoviePrefs.customText ?? ''
  movieRename.sequenceStartNum = 1

  let maxNum = rows.length
  rows.forEach((obj) => {
    const an = movieRename.paneRealSeason ? analyzeEpisodeNumbers(obj.src_name, 2) : analyzeSeasonNumber(obj.src_name, 2)
    if (an.first !== null && an.first > maxNum) maxNum = an.first
  })
  movieRename.sequencePadNum = Math.max(2, String(maxNum).length)
  movieRename.subMode = savedMoviePrefs.subMode ?? 'auto'
  movieRename.subPreset = savedMoviePrefs.subPreset ?? '.简中.zh'
  movieRename.subCustom = savedMoviePrefs.subCustom ?? ''
  movieRename.editingIdx = null
  movieRename.editingName = ''
  movieRename.visible = true
}

// 监听影视重命名选项变化，自动存入 localStorage (方案 A)
watch(
  () => [
    movieRename.prefixMode,
    movieRename.customText,
    movieRename.subMode,
    movieRename.subPreset,
    movieRename.subCustom,
  ],
  () => {
    if (movieRename.visible) {
      saveMovieRenamePrefs()
    }
  }
)

function getRowComputedName(index: number): string {
  const obj = movieRename.items[index]
  if (obj.manual_new_name) return obj.manual_new_name

  let prefix = '',
    finalBase = '',
    injectedSub = ''
  const seqNum = movieRename.sequenceStartNum + index
  const dot = obj.src_name.lastIndexOf('.')
  const ext = !obj.is_dir ? (dot > 0 ? obj.src_name.substring(dot) : '') : ''

  if (REGEX.SUB_EXT.test(obj.src_name) && !obj.is_dir) {
    if (movieRename.subMode === 'preset') injectedSub = movieRename.subPreset === '_blank' ? '' : movieRename.subPreset
    else if (movieRename.subMode === 'custom') injectedSub = movieRename.subCustom
    else if (movieRename.subMode === 'auto') {
      const nameWithoutExt = obj.src_name.substring(0, dot)
      const langMatch = nameWithoutExt.match(REGEX.SUB_LANG)
      if (langMatch) injectedSub = '.' + langMatch[0].substring(1)
    }
  }

  const paddedSeq = String(seqNum).padStart(movieRename.sequencePadNum, '0')
  if (movieRename.paneRealSeason) {
    if (movieRename.prefixMode === 'clean' && movieRename.cleanMediaName) prefix = movieRename.cleanMediaName.trim() + ' '
    else if (movieRename.prefixMode === 'folder' && movieRename.autoFolderName) prefix = movieRename.autoFolderName.trim() + ' '
    else if (movieRename.prefixMode === 'custom') prefix = movieRename.customText
    finalBase =
      movieRename.parseMode === 'sequential'
        ? `${movieRename.paneRealSeason}E${paddedSeq}`
        : `${movieRename.paneRealSeason}E${analyzeEpisodeNumbers(obj.src_name, movieRename.sequencePadNum).numStr}${analyzeEpisodeNumbers(obj.src_name, movieRename.sequencePadNum).extra}`
  } else {
    finalBase =
      movieRename.parseMode === 'sequential'
        ? `Season ${paddedSeq}`
        : `Season ${analyzeSeasonNumber(obj.src_name, movieRename.sequencePadNum).numStr}`
  }
  return obj.is_dir ? `${prefix}${finalBase}` : `${prefix}${finalBase}${injectedSub}${ext}`
}

function rowStatus(index: number) {
  const currentName = getRowComputedName(index)
  const allNames = movieRename.items.map((_, i) => getRowComputedName(i))
  const isDuplicate = allNames.filter((n) => n === currentName).length > 1

  let isSeqBreak = false
  const getNum = (name: string) => {
    const m = movieRename.paneRealSeason ? name.match(/E(\d+)/i) : name.match(/Season\s*(\d+)/i)
    return m ? parseInt(m[1], 10) : null
  }
  if (index > 0) {
    const prevNum = getNum(allNames[index - 1])
    const currNum = getNum(currentName)
    if (prevNum !== null && currNum !== null && currNum !== prevNum + 1) {
      isSeqBreak = true
    }
  }

  let isMismatch = false
  const obj = movieRename.items[index]
  if (movieRename.parseMode === 'regex' && !obj.manual_new_name) {
    const oldAn = movieRename.paneRealSeason ? analyzeEpisodeNumbers(obj.src_name, movieRename.sequencePadNum) : analyzeSeasonNumber(obj.src_name, movieRename.sequencePadNum)
    const newNum = getNum(currentName)
    if (oldAn.first !== null && newNum !== null && oldAn.first !== newNum) isMismatch = true
  }

  return { isDuplicate, isSeqBreak, isMismatch }
}

const movieComputedValidation = computed(() => {
  const allNames = movieRename.items.map((_, i) => getRowComputedName(i))
  const nameCounts: Record<string, number> = {}
  allNames.forEach((n) => (nameCounts[n] = (nameCounts[n] || 0) + 1))
  const hasDuplicates = Object.values(nameCounts).some((c) => c > 1)

  let hasSeqBreak = false
  let hasMismatch = false
  for (let i = 0; i < movieRename.items.length; i++) {
    const st = rowStatus(i)
    if (st.isSeqBreak) hasSeqBreak = true
    if (st.isMismatch) hasMismatch = true
  }

  if (hasDuplicates) {
    return {
      notice: '🚨 发现重复的生成名称！这会导致文件相互覆盖，请检查或修改。',
      noticeClass: 'notice-danger',
    }
  }
  if (hasMismatch) {
    return {
      notice: '❌ 发现提取序号与原文件不符，建议拖拽排序或直接铅笔单行修改。',
      noticeClass: 'notice-pink',
    }
  }
  if (hasSeqBreak && movieRename.parseMode !== 'sequential') {
    return {
      notice: '⚠️ 发现集数跳号或乱序！请检查是否有缺失文件，或直接铅笔单行修改。',
      noticeClass: 'notice-warning',
    }
  }
  if (movieRename.parseMode === 'sequential') {
    return {
      notice: '🔢 强制顺序递增模式已开启！',
      noticeClass: 'notice-info',
    }
  }
  return {
    notice: '✅ 所有校验均通过！',
    noticeClass: 'notice-success',
  }
})

function reverseMovieData() {
  movieRename.items.reverse()
}

let draggedRowIdx: number | null = null
function handleRowDragStart(idx: number, e: DragEvent) {
  draggedRowIdx = idx
  if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move'
}

function handleRowDragOver(idx: number, e: DragEvent) {
  e.preventDefault()
}

function handleRowDrop(idx: number, e: DragEvent) {
  e.preventDefault()
  if (draggedRowIdx !== null && draggedRowIdx !== idx) {
    const moved = movieRename.items.splice(draggedRowIdx, 1)[0]
    movieRename.items.splice(idx, 0, moved)
  }
  draggedRowIdx = null
}

function startInlineEdit(idx: number) {
  movieRename.editingIdx = idx
  movieRename.editingName = getRowComputedName(idx)
}

function saveInlineEdit(idx: number) {
  if (movieRename.editingName.trim()) {
    movieRename.items[idx].manual_new_name = movieRename.editingName.trim()
  }
  movieRename.editingIdx = null
}

function cancelInlineEdit() {
  movieRename.editingIdx = null
}

async function submitMovieRename() {
  movieRename.submitting = true
  const renameObjects = movieRename.items.map((_, i) => ({
    src_name: movieRename.items[i].src_name,
    new_name: getRowComputedName(i),
  }))

  try {
    await callOpenListFsBackend(
      '/api/fs/batch_rename',
      {
        src_dir: movieRename.srcDir,
        rename_objects: renameObjects,
      },
      movieRename.srcDir
    )
    ElMessage.success(`🎉 影视重命名成功！共修改了 ${renameObjects.length} 个项目。`)
    movieRename.visible = false
    renderPane(movieRename.side, true)
  } catch (err: any) {
    ElMessage.error(err.message || '影视重命名失败')
  } finally {
    movieRename.submitting = false
  }
}

// ==========================================
// 模态框 4：🎬 TMDB 智能刮削
// ==========================================
interface TmdbCardResult {
  id: number
  media_type: 'movie' | 'tv'
  displayTitle: string
  displayYear: string
  poster_path?: string
  imdb_id?: string
  tmdbUrl: string
  doubanUrl: string
  imdbUrl: string
  applying?: boolean
}

const tmdbModal = reactive({
  visible: false,
  srcName: '',
  isDir: false,
  ext: '',
  side: 'left' as 'left' | 'right',
  query: '',
  year: '',
  tmdbId: '',
  format: 'standard' as 'standard' | 'full' | 'year' | 'name',
  searching: false,
  error: null as string | null,
  results: [] as TmdbCardResult[],
})

function parseFileInfoForTMDB(filename: string) {
  let name = filename.replace(/\.[a-zA-Z0-9]+$/, '')
  const yearMatch = name.match(/\b(19|20)\d{2}\b/)
  const tmdbMatch = name.match(/tmdbid-(\d+)/i)

  name = name
    .replace(REGEX.CLEANUP, ' ')
    .replace(/-[a-zA-Z0-9]+$/g, ' ')
    .replace(/S\d+E\d+|S\d+/gi, ' ')
    .replace(/\b(19|20)\d{2}\b/g, ' ')
    .replace(/tmdbid-\d+/gi, ' ')
    .replace(/imdbid-[a-z0-9]+/gi, ' ')
    .replace(/[\.\[\]\(\)\-\_\【\】]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()

  return { query: name, year: yearMatch ? yearMatch[0] : '', tmdbId: tmdbMatch ? tmdbMatch[1] : '' }
}

function handleCtxTmdbScrape() {
  if (!ctxMenu.item) return
  const item = ctxMenu.item
  const dot = item.name.lastIndexOf('.')
  const ext = item.is_dir ? '' : dot > 0 ? item.name.substring(dot) : ''
  const parsed = parseFileInfoForTMDB(item.name)

  tmdbModal.side = ctxMenu.side
  tmdbModal.srcName = item.name
  tmdbModal.isDir = item.is_dir
  tmdbModal.ext = ext
  tmdbModal.query = parsed.query
  tmdbModal.year = parsed.year
  tmdbModal.tmdbId = parsed.tmdbId
  const savedTmdb = loadTmdbPrefs()
  tmdbModal.format = savedTmdb.format || 'standard'
  tmdbModal.error = null
  tmdbModal.results = []
  tmdbModal.visible = true
  closeContextMenu()
  performTmdbSearch()
}

// 监听 TMDB 刮削格式变化，自动存入 localStorage (方案 A)
watch(
  () => tmdbModal.format,
  () => {
    if (tmdbModal.visible) {
      saveTmdbPrefs()
    }
  }
)

async function fetchTmdbData(url: string): Promise<any> {
  try {
    const token = TMDB_DEFAULT_TOKEN
    const isBearer = token.length > 50
    const finalUrl = isBearer ? url : url.includes('?') ? `${url}&api_key=${token}` : `${url}?api_key=${token}`
    const headers: Record<string, string> = { Accept: 'application/json' }
    if (isBearer) headers['Authorization'] = `Bearer ${token}`

    const res = await fetch(finalUrl, { headers })
    if (res.ok) return await res.json()
    throw new Error(`HTTP ${res.status}`)
  } catch {
    // 降级使用后端代理请求
    return await proxyTmdbBackend(url, TMDB_DEFAULT_TOKEN)
  }
}

async function performTmdbSearch() {
  const query = tmdbModal.query.trim()
  if (!query && !tmdbModal.tmdbId) return

  tmdbModal.searching = true
  tmdbModal.error = null
  try {
    let url = `https://api.themoviedb.org/3/search/multi?language=zh-CN&query=${encodeURIComponent(query)}`
    if (tmdbModal.year) {
      url += `&primary_release_year=${tmdbModal.year}&first_air_date_year=${tmdbModal.year}`
    }

    const data = await fetchTmdbData(url)
    const rawResults = (data.results || []).filter((r: any) => r.media_type === 'movie' || r.media_type === 'tv').slice(0, 10)

    if (rawResults.length === 0) {
      tmdbModal.error = '未找到结果，请修改名称或年份重新搜索'
      tmdbModal.results = []
      return
    }

    // 抓取外部 IMDb ID
    const imdbIds = await Promise.all(
      rawResults.map(async (r: any) => {
        try {
          const extData = await fetchTmdbData(`https://api.themoviedb.org/3/${r.media_type}/${r.id}/external_ids`)
          return extData.imdb_id || ''
        } catch {
          return ''
        }
      })
    )

    tmdbModal.results = rawResults.map((r: any, idx: number) => {
      const title = r.media_type === 'movie' ? r.title : r.name
      const date = r.release_date || r.first_air_date || ''
      const rYear = date ? date.substring(0, 4) : ''
      const imdbId = imdbIds[idx] || ''

      const tmdbUrl = `https://www.themoviedb.org/${r.media_type}/${r.id}`
      const imdbUrl = imdbId
        ? `https://www.imdb.com/title/${imdbId}/`
        : `https://www.imdb.com/find/?q=${encodeURIComponent(title + (rYear ? ' ' + rYear : ''))}`
      const doubanQuery = imdbId || `${title} ${rYear}`.trim()
      const doubanUrl = `https://search.douban.com/movie/subject_search?search_text=${encodeURIComponent(doubanQuery)}`

      return {
        id: r.id,
        media_type: r.media_type,
        displayTitle: title,
        displayYear: rYear,
        poster_path: r.poster_path,
        imdb_id: imdbId,
        tmdbUrl,
        doubanUrl,
        imdbUrl,
        applying: false,
      }
    })
  } catch (err: any) {
    tmdbModal.error = err.message || 'TMDB 网络异常'
  } finally {
    tmdbModal.searching = false
  }
}

function getTmdbTargetName(card: TmdbCardResult): string {
  const safeTitle = card.displayTitle.replace(/[\/\\:\*\?"<>\|]/g, ' ')
  const ext = tmdbModal.ext
  const imdbString = card.imdb_id ? ` [imdbid-${card.imdb_id}]` : ''

  if (tmdbModal.format === 'name') return `${safeTitle}${ext}`
  if (tmdbModal.format === 'year') return `${safeTitle} ${card.displayYear ? `(${card.displayYear})` : ''}`.trim() + ext
  if (tmdbModal.format === 'full') {
    return `${safeTitle} ${card.displayYear ? `(${card.displayYear})` : ''} [tmdbid-${card.id}]${imdbString}`.trim() + ext
  }
  return `${safeTitle} ${card.displayYear ? `(${card.displayYear})` : ''} [tmdbid-${card.id}]`.trim() + ext
}

async function applyTmdbRename(card: TmdbCardResult) {
  card.applying = true
  const newName = getTmdbTargetName(card)
  const parent = panes[tmdbModal.side].path
  const fullPath = parent === '/' ? `/${tmdbModal.srcName}` : `${parent}/${tmdbModal.srcName}`

  try {
    await callOpenListFsBackend('/api/fs/rename', { path: fullPath, name: newName }, parent)
    ElMessage.success(`✅ TMDB 刮削命名成功: ${newName}`)
    tmdbModal.visible = false
    renderPane(tmdbModal.side, true)
  } catch (err: any) {
    ElMessage.error(err.message || '重命名失败')
  } finally {
    card.applying = false
  }
}

onMounted(() => {
  window.addEventListener('resize', () => {
    isMobile.value = window.innerWidth < 768
  })
})
</script>

<style scoped>
.omni-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  background: rgba(0, 0, 0, 0.55);
  z-index: 9998;
  display: flex;
  justify-content: center;
  align-items: center;
  backdrop-filter: blur(4px);
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  box-sizing: border-box;
}

.omni-modal {
  background: #fff;
  border-radius: 12px;
  width: 98%;
  max-width: 1250px;
  height: 88vh;
  display: flex;
  flex-direction: column;
  box-shadow: 0 12px 40px rgba(0, 0, 0, 0.3);
  overflow: hidden;
  position: relative;
  color: #1e293b;
}

/* 顶部移动端 Tab 栏 */
.omni-mtabs {
  display: none;
}

.omni-body {
  display: flex;
  flex-direction: row;
  flex: 1;
  overflow: hidden;
  min-height: 0;
}

.omni-panel {
  width: 50%;
  display: flex;
  flex-direction: column;
  padding: 15px 15px 5px 15px;
  overflow: hidden;
  box-sizing: border-box;
  min-height: 0;
}

.omni-panel-left {
  border-right: 1px solid #e2e8f0;
}

.omni-list-header {
  display: flex;
  justify-content: space-between;
  margin-bottom: 8px;
  font-size: 14px;
  align-items: center;
  height: 28px;
}

.omni-tools {
  display: flex;
  gap: 8px;
}

.tool-btn {
  background: #fff;
  border: 1px solid #1890ff;
  color: #1890ff;
  padding: 4px 10px;
  border-radius: 4px;
  font-size: 12px;
  cursor: pointer;
  transition: 0.2s;
  display: flex;
  align-items: center;
  gap: 4px;
}
.tool-btn:hover {
  background: #e6f7ff;
}

/* 路径面包屑 */
.omni-path-container {
  display: flex;
  flex-wrap: wrap;
  align-content: flex-start;
  height: 52px;
  min-height: 52px;
  overflow-y: auto;
  background: #f0f5ff;
  border: 1px solid #d6e4ff;
  border-radius: 6px;
  padding: 8px;
  margin-bottom: 10px;
  font-size: 13px;
  line-height: 22px;
}

.omni-crumb {
  cursor: pointer;
  color: #1890ff;
  padding: 2px 4px;
  border-radius: 4px;
  font-weight: bold;
  transition: background 0.2s;
  word-break: break-all;
}
.omni-crumb:hover {
  background: #bae0ff;
}
.omni-crumb-sep {
  color: #999;
  margin: 0 2px;
}

/* 列表区 */
.omni-list {
  flex: 1;
  overflow-y: auto;
  border: 1px solid #e8e8e8;
  border-radius: 8px;
  background: #fafafa;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.omni-empty {
  padding: 30px;
  text-align: center;
  color: #94a3b8;
  font-size: 13px;
}
.text-error {
  color: #ef4444;
}

.omni-item {
  padding: 10px 12px;
  border-bottom: 1px solid #f1f5f9;
  display: flex;
  align-items: center;
  cursor: pointer;
  font-size: 14px;
  user-select: none;
  color: #334155;
  transition: background 0.15s;
}
.omni-item:hover {
  background: #f0f5ff;
}
.omni-item.omni-item-active {
  background: #e6f4ff !important;
  border-left: 3px solid #1890ff !important;
}

.omni-item-title {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  margin-right: 8px;
}

.omni-cb {
  margin-right: 12px;
  transform: scale(1.15);
  cursor: pointer;
  flex-shrink: 0;
}

.omni-more {
  display: none;
}

/* 状态筛选栏 */
.omni-status-bar {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 8px 0;
  min-height: 28px;
}

.omni-filter-row {
  display: flex;
  width: 100%;
  gap: 8px;
  align-items: center;
}

.omni-filter-input {
  padding: 4px 8px;
  border: 1px solid #cbd5e1;
  border-radius: 4px;
  font-size: 12px;
  outline: none;
  width: 160px;
  flex-shrink: 0;
  background: #fff;
  color: #333;
}
.omni-filter-input:focus {
  border-color: #1890ff;
  box-shadow: 0 0 0 2px rgba(24, 144, 255, 0.2);
}

.omni-filter-modes {
  display: flex;
  gap: 10px;
  font-size: 12px;
  color: #64748b;
  background: #f1f5f9;
  padding: 4px 10px;
  border-radius: 4px;
  border: 1px solid #e2e8f0;
}
.omni-filter-modes label {
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 3px;
}

.omni-tag-row {
  display: flex;
  width: 100%;
  gap: 6px;
  align-items: center;
  flex-wrap: wrap;
}

.omni-select-all-tag {
  background: #e6f7ff;
  border: 1px solid #91caff;
  padding: 2px 8px;
  border-radius: 12px;
  font-size: 12px;
  cursor: pointer;
  color: #0958d9;
  transition: all 0.2s;
  user-select: none;
  display: inline-flex;
  align-items: center;
  font-weight: bold;
  flex-shrink: 0;
  gap: 4px;
}
.omni-select-all-tag:hover {
  background: #bae0ff;
  border-color: #1890ff;
}

.omni-tags-list {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
  align-items: center;
}

.ext-tag {
  background: #f5f5f5;
  border: 1px solid #d9d9d9;
  padding: 2px 8px;
  border-radius: 12px;
  font-size: 12px;
  cursor: pointer;
  color: #555;
  transition: all 0.2s;
  user-select: none;
  display: inline-flex;
  align-items: center;
}
.ext-tag:hover {
  background: #bae0ff;
  border-color: #1890ff;
  color: #0050b3;
}
.ext-tag.active {
  background: #1890ff;
  color: #fff;
  border-color: #096dd9;
}
.ext-tag.partial {
  background: #e6f7ff;
  color: #0958d9;
  border-color: #91caff;
}

/* 底部全局动作 */
.omni-footer {
  padding: 12px 20px;
  border-top: 1px solid #e2e8f0;
  background: #fbfbfb;
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.omni-actions {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
  justify-content: flex-end;
}

.omni-btn {
  padding: 8px 16px;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  font-weight: bold;
  font-size: 14px;
  transition: opacity 0.2s, background-color 0.2s;
  display: inline-flex;
  align-items: center;
  gap: 4px;
}
.omni-btn:active {
  opacity: 0.8;
}
.omni-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-exit {
  border: 1px solid #1890ff;
  background: #fff;
  color: #1890ff;
}
.btn-exit:hover {
  background: #e6f7ff;
}
.btn-danger {
  background: #ff4d4f;
  color: #fff;
}
.btn-cancel {
  border: 1px solid #d9d9d9;
  background: #f1f5f9;
  color: #334155;
}
.btn-cancel:hover {
  background: #e2e8f0;
}
.btn-success {
  background: #52c41a;
  color: #fff;
}
.btn-primary {
  background: #1890ff;
  color: #fff;
}
.btn-warning {
  background: #faad14;
  color: #fff;
}
.btn-purple {
  background: #8b5cf6;
  color: #fff;
}

/* 上下文菜单 */
.omni-menu-mask {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  z-index: 99999;
}
.omni-context-menu {
  position: fixed;
  background: #fff;
  border: 1px solid #e2e8f0;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.15);
  border-radius: 8px;
  z-index: 100000;
  min-width: 170px;
  font-size: 13px;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  padding: 4px 0;
}
.omni-ctx-header {
  display: none;
}
.omni-menu-item {
  padding: 10px 16px;
  cursor: pointer;
  color: #334155;
  display: flex;
  align-items: center;
  gap: 8px;
  transition: background 0.1s;
  user-select: none;
}
.omni-menu-item:hover {
  background: #f0f5ff;
  color: #1890ff;
}
.omni-menu-item.danger:hover {
  background: #fef2f2;
  color: #ef4444;
}

/* 子模态窗 */
.sub-modal-mask {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  background: rgba(0, 0, 0, 0.6);
  backdrop-filter: blur(4px);
  z-index: 100001;
  display: flex;
  align-items: center;
  justify-content: center;
}

.sub-modal-box {
  background: #fff;
  border-radius: 12px;
  box-shadow: 0 12px 30px rgba(0, 0, 0, 0.25);
  overflow: hidden;
  display: flex;
  flex-direction: column;
  max-width: 95vw;
  max-height: 88vh;
  padding: 20px;
  box-sizing: border-box;
}
.single-modal-box { width: 400px; }
.batch-modal-box { width: 650px; }
.movie-modal-box { width: 860px; max-height: 90vh; }
.tmdb-modal-box { width: 780px; max-height: 88vh; }
.delete-modal-box { width: 420px; }
.mkdir-modal-box { width: 400px; }
.modal-box-flush { padding: 0 !important; }

.rn-header {
  padding-bottom: 12px;
  border-bottom: 1px solid #e2e8f0;
  font-weight: bold;
  font-size: 16px;
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.rn-close-x {
  cursor: pointer;
  color: #94a3b8;
  font-size: 18px;
}
.rn-close-x:hover {
  color: #ef4444;
}
.rn-body {
  padding: 15px 0 0;
}
.rn-radio-group {
  display: flex;
  gap: 20px;
  margin-bottom: 15px;
  font-size: 14px;
  color: #334155;
}
.rn-radio-label {
  display: flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
}
.rn-desc {
  font-size: 13px;
  color: #64748b;
  margin-bottom: 15px;
  line-height: 1.5;
}
.rn-input {
  width: 100%;
  padding: 10px 12px;
  border: 1px solid #cbd5e1;
  border-radius: 6px;
  box-sizing: border-box;
  font-size: 14px;
  margin-bottom: 12px;
  outline: none;
  color: #1e293b;
  background: #fff;
}
.rn-input:focus {
  border-color: #1890ff;
}

.rn-table-container {
  flex: 1;
  overflow-y: auto;
  border: 1px solid #e2e8f0;
  border-radius: 6px;
  margin-bottom: 15px;
}
.rn-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
  text-align: left;
  table-layout: fixed;
}
.rn-table th,
.rn-table td {
  padding: 10px;
  border-bottom: 1px solid #f1f5f9;
  word-break: break-all;
}
.rn-table th {
  background: #f8fafc;
  position: sticky;
  top: 0;
  font-weight: bold;
  color: #475569;
  z-index: 1;
}

/* 影视重命名专属 */
.movie-settings-details {
  background: #fff;
  border-bottom: 1px solid #e2e8f0;
}
.movie-rows-container {
  flex: 1;
  overflow-y: auto;
  padding: 0 20px 15px 20px;
}
.draggable-row {
  display: flex;
  padding: 10px 10px;
  border-bottom: 1px solid #f3f4f6;
  align-items: center;
  font-size: 13px;
  background: #fff;
  user-select: none;
}
.draggable-row.row-dup {
  background: #fff1f0;
}
.draggable-row.row-err {
  background: #fdf2f8;
}
.draggable-row.row-break {
  background: #fffbe6;
}

.drag-handle {
  width: 36px;
  color: #94a3b8;
  cursor: grab;
  font-size: 18px;
  display: flex;
  align-items: center;
  justify-content: center;
}
.row-seq {
  width: 40px;
  color: #9ca3af;
  font-weight: bold;
  text-align: center;
}
.row-src {
  flex: 1;
  padding-right: 15px;
  color: #4b5563;
  word-break: break-all;
}
.row-arrow {
  width: 60px;
  text-align: center;
  font-size: 13px;
}
.row-new {
  flex: 1;
  padding-left: 15px;
  color: #10b981;
  font-weight: bold;
  word-break: break-all;
  display: flex;
  align-items: center;
}

.badge-ok {
  color: #10b981;
}
.badge-dup {
  color: #cf1322;
  font-weight: bold;
}
.badge-err {
  color: #be185d;
  font-weight: bold;
}
.badge-warn {
  color: #d46b08;
  font-weight: bold;
}
.badge-custom {
  color: #0f766e;
  font-weight: bold;
}

.notice-danger {
  background: #fef2f2;
  border: 1px solid #fca5a5;
  color: #991b1b;
}
.notice-pink {
  background: #fdf2f8;
  border: 1px solid #f9a8d4;
  color: #be185d;
}
.notice-warning {
  background: #fffbeb;
  border: 1px solid #fde047;
  color: #b45309;
}
.notice-info {
  background: #f0f9ff;
  border: 1px solid #bae6fd;
  color: #0369a1;
}
.notice-success {
  background: #f0fdf4;
  border: 1px solid #bbf7d0;
  color: #166534;
}

.inline-actions {
  display: flex;
  align-items: center;
  margin-left: 8px;
  flex-shrink: 0;
}
.inline-edit-btn,
.inline-reset-btn {
  cursor: pointer;
  margin-left: 6px;
  font-size: 14px;
}
.inline-edit-input {
  flex: 1;
  padding: 4px 8px;
  border: 1px solid #1890ff;
  border-radius: 4px;
  font-size: 13px;
  outline: none;
}
.inline-edit-actions {
  display: flex;
  align-items: center;
  margin-left: 6px;
}
.inline-save-btn,
.inline-cancel-btn {
  cursor: pointer;
  margin-left: 6px;
  font-size: 15px;
}

/* TMDB 刮削 */
.tmdb-ext-header {
  padding: 15px 20px;
  background: #f8fafc;
  border-bottom: 1px solid #e2e8f0;
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.tmdb-ext-body {
  padding: 20px;
  flex: 1;
  overflow-y: auto;
  background: #f1f5f9;
}
.tmdb-ext-search-area {
  background: #fff;
  padding: 15px;
  border-radius: 10px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.05);
  margin-bottom: 15px;
  border: 1px solid #e2e8f0;
}
.tmdb-ext-search-row {
  display: flex;
  gap: 10px;
  margin-bottom: 12px;
}
.tmdb-ext-input {
  flex: 1;
  padding: 10px 15px;
  border: 1px solid #cbd5e1;
  border-radius: 6px;
  font-size: 14px;
  outline: none;
}
.tmdb-ext-input-year {
  width: 100px;
  padding: 10px;
  border: 1px solid #cbd5e1;
  border-radius: 6px;
  font-size: 14px;
  text-align: center;
  outline: none;
}
.tmdb-ext-btn {
  padding: 10px 20px;
  background: #3b82f6;
  color: #fff;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  font-weight: bold;
}
.tmdb-ext-format-row {
  display: flex;
  gap: 12px;
  align-items: center;
  font-size: 13px;
  color: #334155;
  flex-wrap: wrap;
}
.tmdb-ext-format-row label {
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 4px;
  font-weight: bold;
}
.tmdb-ext-result-card {
  display: flex;
  background: #fff;
  border-radius: 10px;
  padding: 12px;
  margin-bottom: 12px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.05);
  border: 1px solid #e2e8f0;
  align-items: center;
  gap: 15px;
}
.tmdb-ext-card-content {
  display: flex;
  align-items: center;
  gap: 15px;
  flex: 1;
  min-width: 0;
}
.tmdb-ext-poster {
  width: 70px;
  height: 105px;
  border-radius: 6px;
  object-fit: cover;
  background: #e2e8f0;
  flex-shrink: 0;
}
.tmdb-ext-info {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
.tmdb-ext-title {
  font-size: 16px;
  font-weight: bold;
  color: #0f172a;
  margin: 0 0 4px 0;
}
.tmdb-ext-meta {
  font-size: 12px;
  color: #64748b;
  margin: 0 0 8px 0;
}
.tmdb-ext-preview-row {
  font-size: 13px;
  color: #475569;
  background: #f0fdf4;
  padding: 6px 10px;
  border-radius: 6px;
  border: 1px dashed #bbf7d0;
  word-break: break-all;
}
.tmdb-ext-preview-text {
  color: #166534;
  font-weight: bold;
  font-family: monospace;
}
.tmdb-ext-actions {
  display: flex;
  flex-direction: column;
  gap: 8px;
  align-items: stretch;
  flex-shrink: 0;
  min-width: 140px;
}
.tmdb-ext-apply-btn {
  background: #10b981;
  color: #fff;
  padding: 8px 16px;
  border-radius: 6px;
  border: none;
  font-weight: bold;
  cursor: pointer;
  text-align: center;
}
.tmdb-ext-link-group {
  display: flex;
  gap: 5px;
  width: 100%;
}
.tmdb-ext-link-btn {
  flex: 1;
  padding: 5px 2px;
  border-radius: 4px;
  font-size: 11px;
  font-weight: bold;
  text-align: center;
  text-decoration: none;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  white-space: nowrap;
}
.tmdb-btn-tmdb {
  background: #01b4e4;
  color: #fff;
}
.tmdb-btn-douban {
  background: #007722;
  color: #fff;
}
.tmdb-btn-imdb {
  background: #f5c518;
  color: #000;
}

/* ================= 移动端适配 (<768px) ================= */
@media (max-width: 767px) {
  .omni-overlay {
    background: #fff;
    backdrop-filter: none;
  }
  .omni-modal {
    width: 100%;
    max-width: none;
    height: 100%;
    border-radius: 0;
    box-shadow: none;
  }
  .omni-mtabs {
    display: flex;
    align-items: stretch;
    gap: 6px;
    padding: 8px 10px;
    background: #f8fafc;
    border-bottom: 1px solid #e2e8f0;
    flex-shrink: 0;
  }
  .omni-mtab {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 8px 6px;
    border: 1px solid #d9d9d9;
    background: #fff;
    border-radius: 8px;
    font-size: 14px;
    color: #475569;
    cursor: pointer;
  }
  .omni-mtab.active {
    background: #1890ff;
    border-color: #1890ff;
    color: #fff;
  }
  .omni-mtab .tab-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 90px;
    font-size: 12px;
  }
  .omni-badge {
    min-width: 18px;
    padding: 0 5px;
    height: 18px;
    line-height: 18px;
    border-radius: 9px;
    background: #ff4d4f;
    color: #fff;
    font-size: 11px;
    font-style: normal;
    text-align: center;
  }
  .omni-mexit {
    width: 44px;
    border: 1px solid #d9d9d9;
    background: #fff;
    border-radius: 8px;
    font-size: 16px;
    color: #64748b;
    cursor: pointer;
  }

  .omni-modal[data-tab='left'] .omni-panel-right,
  .omni-modal[data-tab='right'] .omni-panel-left {
    display: none;
  }
  .omni-panel {
    width: 100%;
    padding: 10px 10px 0;
    border: none !important;
  }
  .omni-more {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    font-size: 18px;
    color: #64748b;
  }

  .omni-footer {
    flex-direction: column;
    align-items: stretch;
    gap: 8px;
    padding: 8px 10px;
  }
  .omni-footer > div:first-child {
    display: none;
  }
  .omni-actions {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: 6px;
  }
  .omni-actions .omni-btn {
    flex-direction: column;
    padding: 8px 0;
    font-size: 11px;
    min-height: 50px;
    justify-content: center;
  }

  .omni-context-menu {
    left: 0 !important;
    right: 0 !important;
    top: auto !important;
    bottom: 0 !important;
    width: 100% !important;
    border-radius: 18px 18px 0 0;
    padding-bottom: 20px;
  }
  .omni-ctx-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 18px;
    background: #f8fafc;
    border-bottom: 1px solid #e2e8f0;
  }

  /* 子模态窗全屏/居中适配 */
  .sub-modal-box,
  .single-modal-box,
  .batch-modal-box,
  .movie-modal-box,
  .tmdb-modal-box,
  .delete-modal-box,
  .mkdir-modal-box {
    width: 95vw !important;
    max-width: 95vw !important;
    max-height: 94vh !important;
    padding: 12px !important;
    box-sizing: border-box !important;
    margin: 0 !important;
  }
  .modal-box-flush {
    padding: 0 !important;
  }

  /* 🎬 TMDB 刮削模态框移动端适配 (彻底解决两张截图中的挤压错乱) */
  .tmdb-ext-header {
    padding: 10px 14px !important;
  }
  .tmdb-ext-body {
    padding: 10px !important;
  }
  .tmdb-ext-search-area {
    padding: 10px !important;
    margin-bottom: 10px !important;
  }
  .tmdb-ext-search-row {
    display: flex !important;
    flex-wrap: wrap !important;
    gap: 8px !important;
    margin-bottom: 8px !important;
  }
  .tmdb-ext-input {
    width: 100% !important;
    flex: 1 1 100% !important;
    padding: 8px 12px !important;
    font-size: 14px !important;
    box-sizing: border-box !important;
  }
  .tmdb-ext-input-year {
    flex: 1 1 auto !important;
    width: auto !important;
    min-width: 80px !important;
    padding: 8px 10px !important;
    font-size: 13px !important;
    box-sizing: border-box !important;
  }
  .tmdb-ext-btn {
    flex: 1 1 auto !important;
    padding: 8px 16px !important;
    font-size: 13px !important;
    white-space: nowrap !important;
  }
  .tmdb-ext-format-row {
    font-size: 12px !important;
    gap: 8px !important;
  }
  .tmdb-ext-results-container {
    padding: 8px 0 !important;
  }
  .tmdb-ext-result-card {
    flex-direction: column !important;
    align-items: stretch !important;
    gap: 10px !important;
    padding: 10px !important;
  }
  .tmdb-ext-card-content {
    display: flex !important;
    align-items: flex-start !important;
    gap: 10px !important;
    width: 100% !important;
  }
  .tmdb-ext-poster {
    width: 60px !important;
    height: 90px !important;
    flex-shrink: 0 !important;
  }
  .tmdb-ext-info {
    flex: 1 !important;
    min-width: 0 !important;
  }
  .tmdb-ext-title {
    font-size: 14px !important;
    line-height: 1.3 !important;
  }
  .tmdb-ext-meta {
    font-size: 11px !important;
    margin-bottom: 4px !important;
  }
  .tmdb-ext-preview-row {
    font-size: 11px !important;
    padding: 4px 6px !important;
  }
  .tmdb-ext-actions {
    width: 100% !important;
    min-width: 0 !important;
    flex-direction: column !important;
    gap: 8px !important;
  }
  .tmdb-ext-link-group {
    width: 100% !important;
    display: flex !important;
    gap: 6px !important;
  }
  .tmdb-ext-link-btn {
    flex: 1 !important;
    padding: 6px 4px !important;
    font-size: 12px !important;
    text-align: center !important;
  }
  .tmdb-ext-apply-btn {
    width: 100% !important;
    padding: 8px 12px !important;
    font-size: 13px !important;
  }

  /* 智能校验面板移动端适配 */
  .modal-header-row {
    display: none !important;
  }
  .draggable-row {
    flex-wrap: wrap !important;
    padding: 8px 10px !important;
    gap: 4px !important;
  }
  .row-src {
    flex: 1 1 calc(100% - 65px) !important;
    padding: 0 !important;
    font-size: 12px !important;
    color: #64748b !important;
  }
  .row-arrow {
    display: none !important;
  }
  .row-new {
    flex: 1 1 100% !important;
    padding: 4px 0 0 28px !important;
    font-size: 13px !important;
    color: #0f172a !important;
  }
  .modal-mode-wrapper {
    flex-wrap: wrap !important;
    gap: 8px !important;
  }

  /* 过滤栏移动端适配 */
  .omni-filter-row {
    flex-wrap: wrap !important;
    gap: 6px !important;
  }
  .omni-filter-input {
    flex: 1 1 auto !important;
    width: auto !important;
    min-width: 0 !important;
  }
  .omni-filter-modes {
    gap: 6px !important;
    padding: 4px 6px !important;
    font-size: 11px !important;
  }

  /* 悬浮控制栏层级提升 */
  :deep(.episode-player-bar),
  .episode-player-bar {
    z-index: 100005 !important;
  }
}
</style>
