<template>
  <div class="desktop-shell">
    <aside class="desktop-sidebar">
      <div class="desktop-brand">版舟 <small>RevBoat</small></div>
      <div class="workspace-switcher">
        <button class="repository-switch" :title="repositoryLocation" @click="workspacePickerVisible = true" :aria-label="$t('desktop.switchWorkspace')">
          <span class="repository-mark"><el-icon><Folder /></el-icon></span>
          <span class="repository-copy"><strong>{{ repositoryName }}</strong><small>{{ branchName || $t('workspace.workingCopy') }}</small></span>
          <el-icon><ArrowDown /></el-icon>
        </button>
        <button class="sidebar-add" @click="navigateTo('checkout')" :title="$t('menu.checkout')" :aria-label="$t('menu.checkout')"><el-icon><Plus /></el-icon></button>
      </div>
      <div class="desktop-nav-label">{{ $t('menu.workspace') }}</div>
      <nav class="desktop-nav">
        <button :class="{ active: routeName === 'workspace' || routeName === 'commit' }" @click="navigateTo('workspace')"><el-icon><Document /></el-icon><span>{{ $t('desktop.localChanges') }}</span><b>{{ workspaceStore.statusList.length }}</b></button>
        <button :class="{ active: routeName === 'log' }" @click="navigateTo('log')"><el-icon><RefreshLeft /></el-icon><span>{{ $t('desktop.history') }}</span></button>
        <button :class="{ active: routeName === 'stash' }" @click="navigateTo('stash')"><el-icon><Archive /></el-icon><span>{{ $t('menu.stash') }}</span><b v-if="stashCount">{{ stashCount }}</b></button>
      </nav>
      <div class="desktop-nav-label secondary-label">{{ $t('desktop.tools') }}</div>
      <nav class="desktop-nav secondary-nav">
        <button :class="{ active: routeName === 'checkout' }" @click="navigateTo('checkout')"><el-icon><Download /></el-icon><span>{{ $t('menu.checkout') }}</span></button>
        <button :class="{ active: routeName === 'diff' }" @click="navigateTo('diff')"><el-icon><Connection /></el-icon><span>{{ $t('menu.diff') }}</span></button>
        <button :class="{ active: routeName === 'blame' }" @click="navigateTo('blame')"><el-icon><Edit /></el-icon><span>{{ $t('menu.blame') }}</span></button>
      </nav>
      <div class="desktop-sidebar-bottom"><button @click="navigateTo('settings')" :class="{ active: routeName === 'settings' }"><el-icon><Setting /></el-icon>{{ $t('menu.settings') }}</button><button class="app-version" v-if="updateState === 'available'" @click="openReleasePage" :title="$t('appUpdate.available', {version: latestVersion})">v{{ appVersion }} ↑</button><small v-else>v{{ appVersion }}</small></div>
    </aside>
    <header class="desktop-toolbar">
      <div class="desktop-heading"><strong>{{ routeName === 'checkout' ? $t('desktop.checkoutTitle') : repositoryName }} <span v-if="routeName !== 'checkout' && branchName" class="branch-chip">{{ branchName }}</span></strong><small :title="repositoryLocation">{{ routeName === 'checkout' ? $t('desktop.checkoutHint') : repositoryLocation }}</small></div>
      <div v-if="routeName === 'checkout'" class="desktop-actions"><el-button @click="handleWorkspaceCommand('__open__')"><el-icon><FolderOpened /></el-icon>{{ $t('desktop.addExisting') }}</el-button></div>
      <div v-else class="desktop-actions">
        <button class="toolbar-icon" :disabled="workspaceStore.isLoading || !workspaceStore.currentPath" @click="refreshStatus" :title="$t('menu.refresh')" :aria-label="$t('menu.refresh')"><el-icon :class="{ 'is-loading': workspaceStore.isLoading }"><Refresh /></el-icon></button>
        <WorkspaceOpenMenu :path="workspaceStore.currentPath" compact />
        <button class="toolbar-icon" :disabled="!workspaceStore.currentPath" @click="openTerminal" :title="$t('openIn.terminal')" :aria-label="$t('openIn.terminal')"><span class="terminal-symbol">›_</span></button>
        <span class="toolbar-rule"></span>
        <el-button :disabled="!workspaceStore.currentPath" @click="navigateTo('update')"><el-icon><Download /></el-icon>{{ $t('menu.update') }}</el-button>
        <el-button type="primary" :disabled="!workspaceStore.currentPath" @click="navigateTo('commit')"><el-icon><Upload /></el-icon>{{ $t('menu.commit') }}<span v-if="workspaceStore.statusList.length" class="toolbar-count">{{ workspaceStore.statusList.length }}</span></el-button>
      </div>
    </header>
    <main class="desktop-content" :class="{ 'full-bleed': ['workspace', 'log', 'checkout', 'diff'].includes(routeName) }">
      <div v-if="!['workspace', 'log', 'checkout', 'diff'].includes(routeName)" class="desktop-page-title">{{ currentRouteTitle }}</div>
      <router-view v-slot="{ Component, route }"><keep-alive :include="cachedViews"><component :is="Component" :key="route.path" /></keep-alive></router-view>
    </main>
    <footer class="desktop-status"><span class="status-path" :title="workspaceStore.currentPath || ''"><el-icon><Folder /></el-icon>{{ workspaceStore.currentPath || $t('workspace.noWorkspace') }}</span><span v-if="workspaceStore.svnInfo" class="status-revision">{{ $t('workspace.workingCopy') }} r{{ workspaceStore.svnInfo.revision }}</span><span class="status-summary"><i :class="{ warning: workspaceStore.hasChanges, error: !!workspaceStore.error }"></i>{{ workspaceStore.isLoading ? $t('common.loading') : workspaceStore.error ? $t('common.error') : workspaceStore.hasChanges ? $t('workspace.changeCount', { count: workspaceStore.statusList.length }) : workspaceStore.currentPath ? $t('desktop.localReady') : $t('workspace.noWorkspace') }}</span></footer>
    <el-dialog v-model="workspacePickerVisible" :title="$t('desktop.switchWorkspace')" width="420px" class="workspace-picker-dialog"><WorkspaceSidebar picker @open="handleWorkspaceCommand('__open__')" @select="handleWorkspaceCommand" /></el-dialog>
    <OpenInSettingsDialog />
  </div>
</template>
<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus/es/components/message/index'
import { launchWorkspacePath, openWorkspaceTarget } from '@/api/svn'
import { useWorkspaceStore } from '@/stores/workspace'
import { useStashStore } from '@/stores/stash'
import { useWorkspace } from '@/composables/useWorkspace'
import { useWorkspaceErrorToast } from '@/composables/useWorkspaceErrorToast'
import { useUpdateCheck } from '@/composables/useUpdateCheck'
import WorkspaceSidebar from '@/components/WorkspaceSidebar.vue'
import { Archive } from '@/components/icons/materialIcons'
import WorkspaceOpenMenu from '@/components/WorkspaceOpenMenu.vue'
import OpenInSettingsDialog from '@/components/OpenInSettingsDialog.vue'

const router = useRouter()
const route = useRoute()
const { t } = useI18n()
const workspaceStore = useWorkspaceStore()
const stashStore = useStashStore()
const { loadWorkspace, openWorkspace, refreshStatus, refreshStatusIfStale, openStartupWorkspace } = useWorkspace()
const {
  currentVersion: appVersion,
  state: updateState,
  latestVersion,
  checkForUpdates,
  openReleasePage,
} = useUpdateCheck()
const cachedViews = ref(['WorkspaceView', 'LogView', 'UpdateView'])
const statusRefreshIntervalMs = 60_000
let statusRefreshTimer: number | undefined
useWorkspaceErrorToast()
const stashCount = computed(() => stashStore.currentEntries(workspaceStore.currentPath).length)

const repositoryName = computed(() => {
  const path = workspaceStore.currentPath
  if (!path) return t('workspace.welcomeRepository')
  return path.split(/[\\/]/).filter(Boolean).pop() || path
})

const workspacePickerVisible = ref(false)
const repositoryLocation = computed(() => workspaceStore.svnInfo?.url || workspaceStore.currentPath || t('desktop.checkoutHint'))
const branchName = computed(() => {
  try { return decodeURIComponent(new URL(workspaceStore.svnInfo?.url || '').pathname.split('/').filter(Boolean).pop() || '') } catch { return '' }
})
const openTerminal = async () => {
  if (!workspaceStore.currentPath) return
  try { await openWorkspaceTarget(workspaceStore.currentPath, 'terminal') } catch (error) { ElMessage.error(String(error)) }
}
const routeName = computed(() => String(route.name || 'workspace'))
const currentRouteTitle = computed(() => {
  const titleKey = route.meta.title
  return t(typeof titleKey === 'string' ? titleKey : 'menu.workspace')
})

const navigateTo = (name: string) => router.push({ name })

const handleWorkspaceCommand = async (command: string) => {
  workspacePickerVisible.value = false
  try {
    if (command === '__open__') {
      await openWorkspace(t('workspace.selectWorkspaceTitle'))
    } else if (command !== workspaceStore.currentPath) {
      await loadWorkspace(command)
    }
    await router.push({ name: 'workspace' })
  } catch (error) {
    ElMessage.error(String(error))
  }
}

const refreshStatusSilently = async () => {
  if (!workspaceStore.currentPath || workspaceStore.isLoading) return
  await refreshStatusIfStale(statusRefreshIntervalMs)
}

onMounted(async () => {
  void checkForUpdates()
  await openStartupWorkspace(await launchWorkspacePath().catch(() => null))
  statusRefreshTimer = window.setInterval(refreshStatusSilently, statusRefreshIntervalMs)
})

onUnmounted(() => {
  if (statusRefreshTimer !== undefined) {
    window.clearInterval(statusRefreshTimer)
  }
})
</script>

<style scoped>
.desktop-shell{height:100vh;display:grid;grid-template-columns:216px minmax(0,1fr);grid-template-rows:54px minmax(0,1fr) 28px;overflow:hidden;background:var(--md-sys-color-surface-container-lowest)}
.desktop-sidebar{grid-row:1/-1;background:var(--desktop-sidebar);border-right:1px solid var(--md-sys-color-outline-variant);display:flex;flex-direction:column;min-height:0;padding:0 10px}
.desktop-brand{height:45px;display:flex;align-items:center;justify-content:space-between;padding:0 9px;font-weight:650;letter-spacing:-.3px;color:var(--md-sys-color-on-surface-variant)}.desktop-brand small{font-size:8px;font-weight:500;letter-spacing:1.5px;opacity:.65}
.workspace-switcher{display:flex;gap:5px;margin:6px 0 22px;align-items:stretch}.repository-switch{display:flex;align-items:center;gap:9px;min-width:0;flex:1;border:1px solid var(--md-sys-color-outline-variant);background:var(--md-sys-color-surface-container-lowest);padding:8px;border-radius:8px;box-shadow:0 1px 2px #1d2a3910;cursor:pointer}.repository-switch>.el-icon{font-size:12px;color:var(--md-sys-color-on-surface-variant)}.repository-mark{background:var(--md-sys-color-primary);color:var(--md-sys-color-on-primary);border-radius:6px;display:grid;place-items:center;width:29px;height:30px;flex-shrink:0;font-size:18px}.repository-copy{min-width:0;flex:1;text-align:left}.repository-copy strong,.repository-copy small{display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.repository-copy strong{font-size:12px;font-weight:600}.repository-copy small{color:var(--md-sys-color-on-surface-variant);font-size:10px;margin-top:3px}.sidebar-add{width:27px;background:transparent;border:0;border-radius:7px;cursor:pointer;color:var(--md-sys-color-on-surface-variant)}.sidebar-add:hover{background:var(--md-sys-color-primary-container)}
.desktop-nav-label{font-size:11px;font-weight:550;color:var(--md-sys-color-on-surface-variant);margin:0 9px 8px}.desktop-nav{display:flex;flex-direction:column;gap:3px}.desktop-nav button{width:100%;height:34px;display:flex;align-items:center;gap:10px;padding:0 10px;border:0;border-radius:6px;background:transparent;text-align:left;font-size:13px;cursor:pointer}.desktop-nav button .el-icon{color:var(--md-sys-color-on-surface-variant);font-size:17px}.desktop-nav button:hover,.desktop-sidebar-bottom button:hover{background:var(--md-sys-state-hover)}.desktop-nav button.active{background:var(--md-sys-color-primary-container);color:var(--md-sys-color-on-primary-container);font-weight:600}.desktop-nav b{margin-left:auto;background:var(--desktop-badge);font-weight:500;border-radius:20px;padding:0 6px;font-size:11px}.secondary-label{margin-top:28px}.secondary-nav button{color:var(--md-sys-color-on-surface-variant);font-size:12px}.desktop-sidebar-bottom{margin-top:auto;min-height:44px;border-top:1px solid var(--md-sys-color-outline-variant);display:flex;align-items:center;justify-content:space-between}.desktop-sidebar-bottom button{display:flex;align-items:center;gap:10px;border:0;background:none;padding:8px;border-radius:5px;cursor:pointer}.desktop-sidebar-bottom small,.app-version{font-size:10px;color:var(--md-sys-color-on-surface-variant)}
.desktop-toolbar{display:flex;align-items:center;justify-content:space-between;gap:14px;padding:0 16px;border-bottom:1px solid var(--md-sys-color-outline-variant);background:var(--md-sys-color-surface-container-low)}.desktop-heading{min-width:0}.desktop-heading strong{display:flex;align-items:center;gap:8px;font-size:14px;font-weight:600}.desktop-heading small{display:block;font-size:11px;color:var(--md-sys-color-on-surface-variant);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;margin-top:1px}.branch-chip{font-size:10px;font-weight:500;padding:1px 6px;border-radius:4px;background:var(--md-sys-color-primary-container);color:var(--md-sys-color-on-primary-container)}.desktop-actions{display:flex;align-items:center;gap:8px;flex-shrink:0}.desktop-actions :deep(.el-button+.el-button){margin-left:0}.toolbar-icon{display:grid;place-items:center;width:29px;height:30px;background:transparent;border:0;border-radius:5px;cursor:pointer;font-size:17px;color:var(--md-sys-color-on-surface-variant)}.toolbar-icon:disabled{opacity:.45;cursor:default}.toolbar-icon:hover:enabled{background:var(--md-sys-state-hover)}.terminal-symbol{font:14px monospace;border:1.3px solid currentColor;border-radius:2px;line-height:12px;padding:0 2px}.toolbar-rule{width:1px;height:23px;background:var(--md-sys-color-outline-variant);margin:0 4px}.toolbar-count{display:inline-flex;align-items:center;justify-content:center;font-size:10px;min-width:17px;height:17px;border-radius:20px;background:var(--md-sys-color-on-primary);color:var(--md-sys-color-primary);margin-left:5px}
.desktop-content{min-width:0;min-height:0;overflow:auto;padding:18px}.desktop-content.full-bleed{padding:0;overflow:hidden}.desktop-page-title{font-size:17px;font-weight:600;margin-bottom:15px}.desktop-status{display:flex;gap:15px;align-items:center;padding:0 16px;background:var(--md-sys-color-surface-container-low);border-top:1px solid var(--md-sys-color-outline-variant);font-size:10px;color:var(--md-sys-color-on-surface-variant);min-width:0}.status-path{display:flex;align-items:center;gap:7px;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.status-revision{border-left:1px solid var(--md-sys-color-outline-variant);padding-left:15px;white-space:nowrap}.status-summary{margin-left:auto;display:flex;align-items:center;gap:6px;white-space:nowrap}.status-summary i{width:7px;height:7px;border-radius:50%;background:var(--md-sys-color-success)}.status-summary i.warning{background:var(--md-sys-color-warning)}.status-summary i.error{background:var(--md-sys-color-error)}
@media(max-width:1000px){.desktop-shell{grid-template-columns:190px minmax(0,1fr)}.desktop-heading small{max-width:310px}.desktop-actions{gap:4px}.status-revision{display:none}}
:global(.workspace-picker-dialog .workspace-panel){height:440px;max-height:60vh;width:100%;border:0}:global(.workspace-picker-dialog .el-dialog__body){padding:0}
</style>
