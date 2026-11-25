<template>
  <div class="log-view">
    <div v-if="!workspaceStore.currentPath" class="no-workspace"><el-empty :description="$t('log.openWorkspaceFirst')"><el-button type="primary" @click="openWorkspace">{{ $t('common.open') }}</el-button></el-empty></div>
    <template v-else>
      <section class="history-list-pane">
        <div class="log-filters">
          <el-select
            v-model="filters.author"
            :placeholder="$t('log.searchAuthor')"
            class="author-select"
            filterable
            clearable
            :disabled="authorOptions.length === 0"
            size="small"
            value-on-clear=""
            @clear="clearAuthorFilter"
          >
            <el-option
              v-for="option in authorOptions"
              :key="option.value"
              :label="option.label"
              :value="option.value"
            />
          </el-select>
          <el-input
            v-model="filters.keyword"
            :placeholder="$t('log.searchKeyword')"
            clearable
            size="small"
          />
          <div class="history-date-filter"><el-date-picker
            v-model="dateRange"
            type="daterange"
            value-format="YYYY-MM-DD"
            format="YYYY/MM/DD"
            :start-placeholder="$t('log.dateFrom')"
            :end-placeholder="$t('log.dateTo')"
            range-separator="-"
            popper-class="log-date-range-dropdown"
            placement="bottom-end"
            :fallback-placements="['top-end', 'bottom-start', 'top-start']"
            class="log-date-range"
            clearable
            unlink-panels
            :editable="false"
            size="small"
          />
          </div><el-button type="primary" size="small" :loading="loading" @click="queryLogs">
            <el-icon><Search /></el-icon>
            {{ $t('log.query') }}
          </el-button>
          <el-button size="small" @click="resetFilters">
            <el-icon><RefreshLeft /></el-icon>
            {{ $t('common.reset') }}
          </el-button>
        </div>


        <div class="history-columns"><span>{{ $t('log.revision') }}</span><span>{{ $t('log.commitMessage') }}</span><span>{{ $t('log.author') }}</span><span>{{ $t('log.date') }}</span></div>
        <div ref="logScroller" class="history-scroll" v-loading="loading" :element-loading-text="$t('log.querying')">
          <template v-for="(entry, index) in logs" :key="entry.revision">
            <div v-if="index === 0 || dayLabel(entry.date) !== dayLabel(logs[index - 1].date)" class="history-day">{{ dayLabel(entry.date) }}</div>
            <button class="history-row" :class="{ selected: selectedLog?.revision === entry.revision }" :aria-pressed="selectedLog?.revision === entry.revision" @click="handleRowClick(entry)">
              <span class="history-revision"><i></i>r{{ entry.revision }}</span><span class="history-message" :title="entry.message">{{ entry.message.split('\n')[0] || $t('common.noData') }}</span><span class="history-author" :title="entry.author">{{ entry.author }}</span><time :title="formatDate(entry.date)">{{ new Date(entry.date).toLocaleTimeString(locale, {hour: '2-digit', minute: '2-digit'}) }}</time>
            </button>
          </template>
          <el-empty v-if="!loading && logs.length === 0" :description="hasActiveFilters ? $t('log.noMatchedLogs') : $t('log.noLogs')" />
        </div>
        <footer class="history-pagination"><span>{{ loading ? $t('log.querying') : logResultSummary }}</span><div><el-button text :disabled="loading || pageIndex === 0" @click="changePage(pageIndex - 1)">{{ $t('log.previousPage') }}</el-button><span>{{ $t('log.pageNumber', { page: pageIndex + 1 }) }}</span><el-button text :disabled="loading || !hasMore" @click="changePage(pageIndex + 1)">{{ $t('log.nextPage') }}</el-button></div></footer>
      </section>
      <aside class="history-inspector">
        <template v-if="selectedLog">
          <div class="history-detail-header"><div class="history-version"><strong>r{{ selectedLog.revision }}</strong><el-button text :aria-label="$t('desktop.copyRevision')" :title="$t('desktop.copyRevision')" @click="copyRevision"><el-icon><CopyDocument /></el-icon></el-button></div><h2>{{ selectedLog.message.split('\n')[0] || $t('common.noData') }}</h2><p v-if="selectedLog.message.includes('\n')" class="history-message-body">{{ selectedLog.message.slice(selectedLog.message.indexOf('\n') + 1).trim() }}</p><div class="history-person"><span>{{ selectedLog.author.slice(0,1).toUpperCase() || '?' }}</span><div><strong>{{ selectedLog.author }}</strong><small>{{ formatDate(selectedLog.date) }}</small></div></div></div>
          <div class="history-files-title">{{ $t('log.changedFiles') }} · {{ selectedLog.changed_paths?.length || 0 }}</div>
          <div class="history-files"><button v-for="file in selectedLog.changed_paths" :key="file.path" class="history-file" :class="{ active: selectedChangedPath?.path === file.path }" @click="selectChangedPath(file)" @dblclick="openChangedPathDiff(file)" :title="file.path"><span class="history-status" :class="file.action" :title="getActionLabel(file.action)">{{ file.action }}</span><span class="history-file-name"><strong>{{ fileName(file.path) }}</strong><small>{{ parentPath(file.path) }}</small></span></button></div>
          <div v-if="selectedChangedPath" class="history-preview"><header><strong :title="selectedChangedPath.path">{{ fileName(selectedChangedPath.path) }}</strong><el-button text size="small" @click="openChangedPathDiff(selectedChangedPath)">{{ $t('desktop.fullDiff') }}</el-button></header><div v-if="previewLoading" class="preview-notice">{{ $t('common.loading') }}</div><div v-else-if="previewError" class="preview-notice error">{{ previewError }}</div><template v-else-if="previewText"><div class="preview-stats"><span>+{{ previewStats.added }}</span><span>−{{ previewStats.removed }}</span></div><DiffViewer :text="previewText" compact @stats="previewStats = $event" /></template><div v-else class="preview-notice">{{ $t('diff.noDiff') }}</div></div>
        </template><el-empty v-else :description="$t('desktop.selectRevision')" />
      </aside>
    </template>
  </div>
</template>
<script setup lang="ts">
import { computed, nextTick, onActivated, onUnmounted, reactive, ref, watch } from 'vue'
import { useWorkspaceStore } from '@/stores/workspace'
import { repositoryDiffTarget } from '@/utils/diffTarget'
import { svnCurrentUser, svnLog, svnDiff } from '@/api/svn'
import type { SvnLogEntry, SvnLogPath } from '@/types'
import { useI18n } from 'vue-i18n'
import { useWorkspace } from '@/composables/useWorkspace'
import { useRouter } from 'vue-router'
import DiffViewer from '@/components/DiffViewer.vue'
import { writeClipboardText } from '@/utils/clipboard'
import { ElMessage } from 'element-plus/es/components/message/index'

const { t, locale } = useI18n()
const router = useRouter()
const workspaceStore = useWorkspaceStore()
const { openWorkspace: openWorkspaceDialog } = useWorkspace()

const logs = ref<SvnLogEntry[]>([])
const loading = ref(false)
const hasMore = ref(true)
const logScroller = ref<HTMLElement | null>(null)
let requestGeneration = 0
let authorRequestGeneration = 0
const pageIndex = ref(0)
const pageCursors = ref<(number | undefined)[]>([undefined])
const FILTER_RELOAD_DELAY_MS = 250
let filterReloadTimer: number | undefined
const selectedLog = ref<SvnLogEntry | null>(null)
const LOG_CACHE_TTL_MS = 5 * 60 * 1000
const LOG_CACHE_KEY_VERSION = 'v4'
const DEFAULT_LOG_PAGE_SIZE = 20
const MAX_CACHE_SIZE = 50
type LogCacheEntry = {
  entries: SvnLogEntry[]
  cachedAt: number
}
const logPageCache = new Map<string, LogCacheEntry>()
const logCacheVersion = ref(0)
const currentAuthor = ref('')

const getDefaultFilters = () => ({
  author: '',
  keyword: '',
  dateFrom: '',
  dateTo: '',
})

const filters = reactive({
  ...getDefaultFilters(),
})
const hasActiveFilters = computed(() => {
  return Boolean(filters.author.trim() || filters.keyword.trim() || filters.dateFrom || filters.dateTo)
})
const logResultSummary = computed(() => {
  const key = hasActiveFilters.value ? 'log.matchedCount' : 'log.loadedCount'
  return t(key, { count: logs.value.length })
})

const dateRange = computed<string[]>({
  get: () => filters.dateFrom && filters.dateTo ? [filters.dateFrom, filters.dateTo] : [],
  set: (value: string[]) => {
    filters.dateFrom = value?.[0] || ''
    filters.dateTo = value?.[1] || ''
  },
})

const cloneLogEntry = (entry: SvnLogEntry): SvnLogEntry => ({
  ...entry,
  changed_paths: [...(entry.changed_paths || [])],
})

const sortLogEntries = (entries: SvnLogEntry[]) => {
  return [...entries].sort((a, b) => {
    const revisionOrder = b.revision - a.revision
    if (revisionOrder !== 0) return revisionOrder
    return Date.parse(b.date || '') - Date.parse(a.date || '')
  })
}

const normalizeLogEntries = (entries: SvnLogEntry[]) => {
  const seenRevisions = new Set<number>()
  const uniqueEntries = entries.filter(entry => {
    if (seenRevisions.has(entry.revision)) return false
    seenRevisions.add(entry.revision)
    return true
  })
  return sortLogEntries(uniqueEntries.map(cloneLogEntry))
}

const getLogCacheKey = (path: string, limit: number | undefined, startRev?: number, endRev?: number, keyword?: string, author?: string, dateFrom?: string, dateTo?: string) => {
  return `${LOG_CACHE_KEY_VERSION}|${path}|${limit}|${startRev ?? ''}|${endRev ?? ''}|${keyword ?? ''}|${author ?? ''}|${dateFrom ?? ''}|${dateTo ?? ''}`
}

const getCachedLogPage = (path: string, limit: number | undefined, startRev?: number, endRev?: number, keyword?: string, author?: string, dateFrom?: string, dateTo?: string) => {
  const key = getLogCacheKey(path, limit, startRev, endRev, keyword, author, dateFrom, dateTo)
  const cached = logPageCache.get(key)
  if (!cached) return null
  if (Date.now() - cached.cachedAt > LOG_CACHE_TTL_MS) {
    logPageCache.delete(key)
    return null
  }
  return normalizeLogEntries(cached.entries)
}

const setCachedLogPage = (
  path: string,
  limit: number | undefined,
  entries: SvnLogEntry[],
  startRev?: number,
  endRev?: number,
  keyword?: string,
  author?: string,
  dateFrom?: string,
  dateTo?: string
) => {
  // 缓存达到上限时淘汰最旧的条目
  if (logPageCache.size >= MAX_CACHE_SIZE) {
    let oldestKey: string | undefined
    let oldestTime = Infinity
    for (const [k, v] of logPageCache) {
      if (v.cachedAt < oldestTime) {
        oldestTime = v.cachedAt
        oldestKey = k
      }
    }
    if (oldestKey) logPageCache.delete(oldestKey)
  }

  const key = getLogCacheKey(path, limit, startRev, endRev, keyword, author, dateFrom, dateTo)
  logPageCache.set(key, {
    entries: normalizeLogEntries(entries),
    cachedAt: Date.now(),
  })
  logCacheVersion.value += 1
}

const clearLogCacheForPath = (path: string) => {
  const prefix = `${LOG_CACHE_KEY_VERSION}|${path}|`
  let changed = false
  for (const key of logPageCache.keys()) {
    if (key.startsWith(prefix)) {
      logPageCache.delete(key)
      changed = true
    }
  }
  if (changed) logCacheVersion.value += 1
}

const getCachedAuthorsForPath = (path: string | null) => {
  if (!path) return []
  logCacheVersion.value

  const prefix = `${LOG_CACHE_KEY_VERSION}|${path}|`
  const authors = new Set<string>()
  for (const [key, cached] of logPageCache.entries()) {
    if (!key.startsWith(prefix)) continue
    cached.entries.forEach(entry => {
      if (entry.author) authors.add(entry.author)
    })
  }
  return [...authors]
}

const knownAuthors = computed(() => {
  const authors = new Set<string>()
  logs.value.forEach(entry => {
    if (entry.author) authors.add(entry.author)
  })
  getCachedAuthorsForPath(workspaceStore.currentPath).forEach(author => authors.add(author))
  return [...authors]
})

const authorOptions = computed(() => {
  const self = currentAuthor.value
  const otherAuthors = knownAuthors.value
    .filter(author => author && author !== self)
    .sort((a, b) => a.localeCompare(b, locale.value))

  return [
    ...(self ? [{ label: self, value: self }] : []),
    ...otherAuthors.map(author => ({ label: author, value: author })),
  ]
})

const loadCurrentAuthor = async (path: string) => {
  const generation = ++authorRequestGeneration
  try {
    const authUser = await svnCurrentUser(path)
    if (generation === authorRequestGeneration && path === workspaceStore.currentPath) {
      currentAuthor.value = authUser?.username || ''
    }
  } catch {
    if (generation === authorRequestGeneration && path === workspaceStore.currentPath) {
      currentAuthor.value = ''
    }
  }
}

const openWorkspace = async () => {
  const success = await openWorkspaceDialog(t('dialog.selectSVNWorkspaceDirectory'))
  if (success) {
    reloadLogs()
  }
}

let lastLogError: string | null = null

const fetchLogPage = async (index: number, refreshCache = false) => {
  const requestedPath = workspaceStore.currentPath
  if (!requestedPath) return
  const generation = ++requestGeneration
  const startRev = pageCursors.value[index]
  const endRev = startRev === undefined ? undefined : 1
  // One extra match determines whether there is another page, including filters.
  const limit = DEFAULT_LOG_PAGE_SIZE + 1
  const kw = filters.keyword.trim() || undefined
  const author = filters.author.trim() || undefined
  const df = filters.dateFrom || undefined
  const dt = filters.dateTo || undefined
  loading.value = true
  try {
    const cached = refreshCache ? null : getCachedLogPage(requestedPath, limit, startRev, endRev, kw, author, df, dt)
    const batch = cached || normalizeLogEntries(await svnLog(requestedPath, limit, startRev, endRev, kw, author, df, dt))
    if (generation !== requestGeneration || requestedPath !== workspaceStore.currentPath) return
    if (!cached) setCachedLogPage(requestedPath, limit, batch, startRev, endRev, kw, author, df, dt)
    logs.value = batch.slice(0, DEFAULT_LOG_PAGE_SIZE)
    selectedLog.value = logs.value.find(entry => entry.revision === selectedLog.value?.revision) || logs.value[0] || null
    hasMore.value = batch.length > DEFAULT_LOG_PAGE_SIZE
    pageIndex.value = index
    if (hasMore.value) pageCursors.value[index + 1] = logs.value[logs.value.length - 1].revision - 1
    await nextTick()
    if (logScroller.value) logScroller.value.scrollTop = 0
    if (lastLogError !== null) {
      if (workspaceStore.error === lastLogError) workspaceStore.setError(null)
      lastLogError = null
    }
  } catch (err) {
    if (generation === requestGeneration) {
      lastLogError = String(err)
      workspaceStore.setError(lastLogError)
    }
  } finally {
    if (generation === requestGeneration) loading.value = false
  }
}

const reloadLogs = async (refreshCache = false, restart = false) => {
  if (!restart && !refreshCache && loading.value) return
  if (refreshCache && workspaceStore.currentPath) clearLogCacheForPath(workspaceStore.currentPath)
  pageIndex.value = 0
  pageCursors.value = [undefined]
  logs.value = []
  selectedLog.value = null
  hasMore.value = false
  await fetchLogPage(0, refreshCache)
}

const changePage = (index: number) => {
  if (loading.value || filterReloadTimer !== undefined || index < 0 || index >= pageCursors.value.length) return
  void fetchLogPage(index)
}

const scheduleFilterReload = () => {
  if (!workspaceStore.currentPath) return
  if (filterReloadTimer !== undefined) {
    window.clearTimeout(filterReloadTimer)
  }
  filterReloadTimer = window.setTimeout(() => {
    filterReloadTimer = undefined
    void reloadLogs(true, true)
  }, FILTER_RELOAD_DELAY_MS)
}

const queryLogs = () => {
  if (!workspaceStore.currentPath || loading.value) return
  if (filterReloadTimer !== undefined) {
    window.clearTimeout(filterReloadTimer)
    filterReloadTimer = undefined
  }
  void reloadLogs(true, true)
}

const handleRowClick = (row: SvnLogEntry) => {
  selectedLog.value = row
}



const openChangedPathDiff = (row: SvnLogPath) => {
  if (!selectedLog.value || !workspaceStore.svnInfo) return

  sessionStorage.setItem('orca_log_selection', String(selectedLog.value.revision))

  const repositoryRoot = workspaceStore.svnInfo.repository_root.replace(/\/+$/, '')
  const revision = selectedLog.value.revision
  const currentTarget = repositoryDiffTarget(repositoryRoot, row.path, revision, row.action)
  const allFiles = (selectedLog.value.changed_paths || []).map(p => ({
    display: p.path,
    query: repositoryDiffTarget(repositoryRoot, p.path, revision, p.action),
  }))
  const currentIndex = allFiles.findIndex(f => f.query === currentTarget)

  sessionStorage.setItem('orca_diff_files', JSON.stringify({
    source: 'log',
    files: allFiles.map(f => f.query),
    displayNames: allFiles.map(f => f.display),
    current: currentTarget,
    index: Math.max(0, currentIndex),
  }))

  router.push({
    name: 'diff',
    query: {
      path: currentTarget,
      revision: String(revision),
    },
  })
}

const resetFilters = () => {
  Object.assign(filters, getDefaultFilters())
}

const clearAuthorFilter = () => {
  filters.author = ''
}

const getActionLabel = (action: string) => {
  const key = `log.action${action || 'Unknown'}`
  const translated = t(key)
  return translated === key ? action || t('common.noData') : translated
}

const formatDate = (dateStr: string): string => {
  if (!dateStr) return ''
  const date = new Date(dateStr)
  return date.toLocaleString(locale.value)
}

watch(
  () => workspaceStore.currentPath,
  (path, oldPath) => {
    if (path && path !== oldPath) {
      requestGeneration += 1
      selectedLog.value = null
      loading.value = false
      currentAuthor.value = ''
      loadCurrentAuthor(path)
      reloadLogs(true)
    }
    if (!path) {
      requestGeneration += 1
      authorRequestGeneration += 1
      logs.value = []
      selectedLog.value = null
      currentAuthor.value = ''
      hasMore.value = true
      loading.value = false
      pageIndex.value = 0
      pageCursors.value = [undefined]
    }
  },
  { immediate: true }
)

watch(
  () => [filters.author, filters.keyword, filters.dateFrom, filters.dateTo] as const,
  scheduleFilterReload
)

onActivated(() => {
  const saved = sessionStorage.getItem('orca_log_selection')
  if (saved) {
    const revision = Number(saved)
    if (!isNaN(revision)) {
      const log = logs.value.find(l => l.revision === revision)
      if (log) {
        selectedLog.value = log
      }
    }
    sessionStorage.removeItem('orca_log_selection')
  }
})

onUnmounted(() => {
  sessionStorage.removeItem('orca_log_selection')
  if (filterReloadTimer !== undefined) {
    window.clearTimeout(filterReloadTimer)
  }
})


const selectedChangedPath = ref<SvnLogPath | null>(null)
const previewText = ref('')
const previewError = ref('')
const previewLoading = ref(false)
const previewStats = ref({ added: 0, removed: 0 })
let previewGeneration = 0
const dayLabel = (date: string) => date ? new Date(date).toLocaleDateString(locale.value, { year: 'numeric', month: 'long', day: 'numeric', weekday: 'short' }) : t('common.noData')
const fileName = (path: string) => path.split('/').pop() || path
const parentPath = (path: string) => path.slice(0, path.lastIndexOf('/')) || '/'
async function copyRevision() {
  if (!selectedLog.value) return
  try { await writeClipboardText(String(selectedLog.value.revision)) } catch (error) { ElMessage.error(String(error)) }
}
async function selectChangedPath(path: SvnLogPath | null) {
  const generation = ++previewGeneration
  selectedChangedPath.value = path
  previewText.value = ''
  previewError.value = ''
  previewStats.value = { added: 0, removed: 0 }
  previewLoading.value = false
  const entry = selectedLog.value
  const workspace = workspaceStore.currentPath
  const root = workspaceStore.svnInfo?.repository_root.replace(/\/+$/, '')
  if (!path || !entry || !workspace || !root) return
  previewLoading.value = true
  try {
    const result = await svnDiff(workspace, repositoryDiffTarget(root, path.path, entry.revision, path.action), entry.revision)
    if (generation === previewGeneration && workspace === workspaceStore.currentPath) previewText.value = result.diff
  } catch (error) {
    if (generation === previewGeneration) previewError.value = String(error)
  } finally { if (generation === previewGeneration) previewLoading.value = false }
}
watch(selectedLog, log => { void selectChangedPath(log?.changed_paths?.[0] || null) })

</script>

<style scoped>
.log-view{height:100%;min-height:0;display:flex;background:var(--md-sys-color-surface-container-lowest)}.no-workspace{margin:auto}.history-list-pane{flex:1;min-width:0;display:flex;flex-direction:column}.log-filters{display:flex;flex-wrap:wrap;align-items:center;gap:7px;padding:8px 12px;border-bottom:1px solid var(--md-sys-color-outline-variant);min-height:44px}.log-filters>.el-input{order:-1;flex:1;min-width:140px}.log-filters :deep(.el-input__wrapper){background:var(--md-sys-color-surface-container)}.author-select{width:105px}.log-date-range{max-width:220px!important;flex:0 1 220px!important}.log-filters .el-button+.el-button{margin:0}.history-columns,.history-row{display:grid;grid-template-columns:74px minmax(0,1fr) 85px 54px;align-items:center;gap:10px;padding:0 16px}.history-columns{height:30px;font-size:11px;font-weight:550;color:var(--md-sys-color-on-surface-variant);border-bottom:1px solid var(--md-sys-color-outline-variant)}.history-columns span:last-child{text-align:right}.history-scroll{flex:1;min-height:0;overflow:auto}.history-day{font-size:11px;color:var(--md-sys-color-on-surface-variant);padding:6px 16px;background:var(--md-sys-color-surface-container-low);border-bottom:1px solid var(--md-sys-color-outline-variant)}.history-row{width:100%;height:35px;background:none;border:0;border-bottom:1px solid color-mix(in srgb,var(--md-sys-color-outline-variant) 65%,transparent);text-align:left;cursor:pointer;font-size:12px}.history-row:hover{background:var(--md-sys-color-primary-container)}.history-row.selected{background:var(--desktop-selected);color:var(--desktop-selected-text)}.history-revision{position:relative;display:flex;gap:14px;align-items:center;font-family:var(--app-font-family-mono)}.history-revision:before{content:'';position:absolute;left:4px;top:-12px;bottom:-12px;width:1px;background:var(--md-sys-color-outline-variant)}.history-revision i{position:relative;z-index:1;width:9px;height:9px;border-radius:50%;border:1.5px solid #8c9eb2;background:var(--md-sys-color-surface-container-lowest)}.selected .history-revision i{background:#fff;border-color:#fff}.history-message,.history-author{overflow:hidden;white-space:nowrap;text-overflow:ellipsis}.history-author,.history-row time{font-size:11px;color:var(--md-sys-color-on-surface-variant)}.history-row time{text-align:right}.selected .history-author,.selected time{color:#e1e9f2}.history-pagination{display:flex;justify-content:space-between;align-items:center;gap:8px;height:34px;padding:0 12px;border-top:1px solid var(--md-sys-color-outline-variant);color:var(--md-sys-color-on-surface-variant);font-size:10px}.history-pagination>div{display:flex;gap:5px;align-items:center}.history-pagination .el-button{min-height:26px;padding:3px 5px;font-size:10px}.history-inspector{width:32%;min-width:300px;max-width:430px;border-left:1px solid var(--md-sys-color-outline-variant);background:var(--md-sys-color-surface-container-low);overflow:auto}.history-detail-header{padding:16px;border-bottom:1px solid var(--md-sys-color-outline-variant)}.history-version{display:flex;align-items:center;gap:5px}.history-version>strong{font-size:20px;color:var(--md-sys-color-primary);font-family:var(--app-font-family-mono)}.history-detail-header h2{font-size:14px;line-height:1.65;font-weight:600;margin:12px 0 8px;overflow-wrap:anywhere}.history-message-body{font-size:12px;color:var(--md-sys-color-on-surface-variant);white-space:pre-wrap;line-height:1.8;overflow-wrap:anywhere}.history-person{display:flex;gap:9px;align-items:center;margin-top:17px;font-size:11px}.history-person>span{display:grid;place-items:center;width:30px;height:30px;border-radius:50%;background:var(--md-sys-color-primary-container);color:var(--md-sys-color-primary);font-size:13px}.history-person strong{font-weight:550}.history-person small{display:block;font-size:10px;color:var(--md-sys-color-on-surface-variant);margin-top:3px}.history-files-title{font-size:11px;font-weight:550;padding:12px 16px}.history-files{padding:0 8px}.history-file{display:flex;align-items:center;gap:9px;width:100%;background:none;border:0;border-radius:6px;text-align:left;padding:8px;cursor:pointer}.history-file:hover,.history-file.active{background:var(--md-sys-color-primary-container)}.history-status{display:grid;place-items:center;width:18px;height:19px;border-radius:4px;background:var(--app-color-status-modified-bg);color:var(--app-color-status-modified-text);font-size:10px;flex-shrink:0;font-weight:650}.history-status.A{background:var(--app-color-status-added-bg);color:var(--app-color-status-added-text)}.history-status.D{background:var(--app-color-status-error-bg);color:var(--app-color-status-error-text)}.history-file-name{min-width:0}.history-file-name strong,.history-file-name small{display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.history-file-name strong{font-weight:500;font-size:12px}.history-file-name small{color:var(--md-sys-color-on-surface-variant);font-size:10px;margin-top:2px}.history-preview{margin:16px 12px;border:1px solid var(--md-sys-color-outline-variant);border-radius:7px;overflow:hidden;background:var(--md-sys-color-surface-container-lowest)}.history-preview>header{display:flex;align-items:center;justify-content:space-between;padding:5px 9px;border-bottom:1px solid var(--md-sys-color-outline-variant);gap:8px;font-size:11px}.history-preview>header strong{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.history-preview :deep(.diff-viewer){height:320px}.preview-notice{padding:20px;font-size:11px;color:var(--md-sys-color-on-surface-variant);overflow-wrap:anywhere}.preview-notice.error{color:var(--md-sys-color-error)}.preview-stats{display:flex;gap:12px;padding:5px 10px;font-size:11px}.preview-stats span:first-child{color:var(--md-sys-color-success)}.preview-stats span:last-child{color:var(--md-sys-color-error)}
@media(max-width:1050px){.history-inspector{min-width:260px;width:35%}.history-columns,.history-row{grid-template-columns:58px minmax(0,1fr) 62px;gap:6px;padding:0 12px}.history-row time,.history-columns>span:last-child{display:none}.log-date-range{max-width:190px!important;flex-basis:190px!important}}
.history-date-filter{flex:0 0 220px;min-width:0}.history-date-filter :deep(.el-date-editor){width:100%!important;min-width:0!important;font-size:11px}.history-date-filter :deep(.el-range-input){font-size:11px}.history-preview :deep(.diff-viewer){border:0;border-radius:0}
@media(max-width:1050px){.history-date-filter{flex-basis:190px}}
</style>
