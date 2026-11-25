<template>
  <div class="checkout-view">
    <div class="checkout-main">
      <el-form :model="form" label-width="100px" class="checkout-form" label-position="right" :disabled="loading" @submit.prevent="doCheckout">
        <el-form-item :label="$t('checkout.repositoryUrl')" required>
          <div class="url-control"><el-input v-model="form.url" :placeholder="$t('checkout.repositoryUrlPlaceholder')" clearable><template #prefix><el-icon><Link /></el-icon></template></el-input><el-button :loading="connecting" :disabled="!form.url.trim()" @click="testConnection">{{ $t('desktop.testConnection') }}</el-button></div>
          <div v-if="connectionInfo" class="connection-result"><el-icon><CircleCheck /></el-icon>{{ $t('desktop.connected') }} · r{{ connectionInfo.revision }}</div>
          <div v-if="connectionError" class="connection-error" role="alert">{{ connectionError }}</div>
        </el-form-item>
        <el-form-item :label="$t('desktop.remoteDirectory')" class="remote-form-item">
          <section class="remote-browser" v-loading="browsing">
            <div class="remote-breadcrumb"><el-button text size="small" :disabled="!canGoUp || browsing" @click="goUp"><el-icon><ArrowLeft /></el-icon>{{ $t('desktop.parentDirectory') }}</el-button><span :title="browseUrl">{{ browseUrl || $t('desktop.connectFirst') }}</span></div>
            <div class="remote-columns"><span>{{ $t('desktop.name') }}</span><span>{{ $t('log.revision') }}</span><span>{{ $t('log.date') }}</span></div>
            <div class="remote-rows">
              <button v-if="connectionInfo" type="button" class="remote-entry current-directory" :class="{ selected: !selectedEntry }" @click="selectedEntry = null" :disabled="loading"><span><el-icon><Folder /></el-icon>{{ $t('desktop.currentDirectory') }}</span><small>—</small><small>—</small></button>
              <button v-for="entry in entries" :key="entry.name" type="button" class="remote-entry" :class="{ selected: selectedEntry?.name === entry.name }" :disabled="entry.kind !== 'dir' || loading" @click="selectedEntry = entry" @dblclick="enterDirectory(entry)" @keydown.enter.prevent="enterDirectory(entry)" @keydown.right.prevent="enterDirectory(entry)"><span><el-icon><Folder v-if="entry.kind === 'dir'" /><Document v-else /></el-icon>{{ entry.name }}</span><small>r{{ entry.revision }}</small><small>{{ shortDate(entry.date) }}</small></button>
              <div v-if="!connectionInfo" class="remote-empty"><el-icon><FolderOpened /></el-icon><p>{{ $t('desktop.connectFirst') }}</p></div>
              <p v-else-if="!entries.length && !browsing" class="empty-directory">{{ $t('desktop.emptyDirectory') }}</p>
            </div>
          </section>
          <small class="field-hint">{{ $t('desktop.remoteHint') }}</small>
        </el-form-item>
        <el-form-item :label="$t('checkout.targetDirectory')" required><div class="url-control"><el-input v-model="form.path" :placeholder="$t('checkout.selectTargetDirectory')" clearable /><el-button @click="selectDirectory">{{ $t('common.browse') }}…</el-button></div></el-form-item>
        <el-form-item :label="$t('checkout.revision')"><div class="revision-controls"><el-radio-group v-model="revisionMode"><el-radio-button value="head">{{ $t('desktop.latestRevision') }}</el-radio-button><el-radio-button value="custom">{{ $t('desktop.customRevision') }}</el-radio-button></el-radio-group><el-input v-if="revisionMode === 'custom'" v-model.number="form.revision" type="number" min="0" :placeholder="$t('log.revision')" class="revision-number" /></div></el-form-item>
        <el-form-item :label="$t('desktop.depth')"><el-select v-model="form.depth" class="depth-select"><el-option v-for="depth in ['infinity','immediates','files','empty']" :key="depth" :value="depth" :label="$t(`desktop.depth_${depth}`)" /></el-select></el-form-item>
        <el-form-item><div class="checkout-options"><label><input type="checkbox" v-model="form.ignoreExternals" :disabled="loading">{{ $t('desktop.ignoreExternals') }}</label><label><input type="checkbox" v-model="form.switchAfter" :disabled="loading">{{ $t('desktop.switchAfter') }}</label></div></el-form-item>
        <div v-if="output" class="output-area" role="status"><div class="output-header"><strong>{{ $t('checkout.output') }}</strong><el-tag :type="outputSucceeded ? 'success' : 'danger'">{{ outputSucceeded ? $t('common.success') : $t('common.error') }}</el-tag></div><pre>{{ output }}</pre></div>
      </el-form>
      <footer class="checkout-actions form-actions"><span>{{ loading ? $t('desktop.checkingOut') : $t('desktop.checkoutReady') }}</span><el-button :disabled="loading" @click="resetForm">{{ $t('common.reset') }}</el-button><el-button type="primary" :loading="loading" :disabled="connecting || browsing" @click="doCheckout"><el-icon><Download /></el-icon>{{ $t('checkout.startCheckout') }}</el-button></footer>
    </div>
    <aside class="checkout-inspector">
      <div class="directory-summary"><small>{{ $t('desktop.selectedDirectory') }}</small><div class="selected-folder"><span><el-icon><Folder /></el-icon></span><div><strong>{{ selectedName }}</strong><small :title="checkoutUrl">{{ checkoutUrl || $t('desktop.notConnected') }}</small></div></div></div>
      <dl><dt>{{ $t('desktop.repositoryRevision') }}</dt><dd>{{ connectionInfo ? `r${connectionInfo.revision}` : '—' }}</dd><dt>{{ $t('desktop.lastChanged') }}</dt><dd>{{ selectedEntry ? `r${selectedEntry.revision}` : '—' }}</dd><dt>{{ $t('log.author') }}</dt><dd>{{ selectedEntry?.author || '—' }}</dd><dt>{{ $t('desktop.protocol') }}</dt><dd>{{ protocol || '—' }}</dd><dt>{{ $t('checkout.revision') }}</dt><dd>{{ revisionMode === 'head' ? 'HEAD' : form.revision === undefined ? '—' : `r${form.revision}` }}</dd></dl>
      <div class="checkout-help"><h3>{{ $t('desktop.guiAuthentication') }}</h3><p>{{ $t('desktop.guiAuthenticationHint') }}</p></div>
      <div class="checkout-session"><el-icon><User /></el-icon><div><strong>{{ $t('svnAuth.title') }}</strong><p>{{ $t(credentialStorageHint()) }}</p></div></div>
    </aside>
  </div>
</template>
<script setup lang="ts">
import { credentialStorageHint } from '@/api/auth'
import { computed, onUnmounted, reactive, ref, watch } from 'vue'
import { onBeforeRouteLeave, useRouter } from 'vue-router'
import { svnCheckout, svnConnectionInfo, svnListRemote } from '@/api/svn'
import { useWorkspace } from '@/composables/useWorkspace'
import { useWorkspaceStore } from '@/stores/workspace'
import { open } from '@tauri-apps/plugin-dialog'
import { useI18n } from 'vue-i18n'
import type { SvnInfo } from '@/types'
import { parseRemoteEntries, childRepositoryUrl } from '@/utils/remoteBrowser'
const { t, locale } = useI18n()
const router = useRouter()
const { loadWorkspace } = useWorkspace()
const store = useWorkspaceStore()
const form = reactive({ url: '', path: '', revision: undefined as number | undefined, depth: 'infinity', ignoreExternals: false, switchAfter: true })
const revisionMode = ref('head')
const loading = ref(false), connecting = ref(false), browsing = ref(false)
const output = ref(''), outputSucceeded = ref(false), connectionError = ref('')
const connectionInfo = ref<SvnInfo | null>(null)
const entries = ref<ReturnType<typeof parseRemoteEntries>>([])
const selectedEntry = ref<ReturnType<typeof parseRemoteEntries>[number] | null>(null)
const browseUrl = ref(''), rootUrl = ref('')
let generation = 0
const checkoutUrl = computed(() => selectedEntry.value ? childRepositoryUrl(browseUrl.value, selectedEntry.value.name) : browseUrl.value || form.url.trim())
const selectedName = computed(() => { try { return decodeURIComponent(new URL(checkoutUrl.value).pathname.split('/').filter(Boolean).pop() || '/') } catch { return t('desktop.notConnected') } })
const protocol = computed(() => { try { return new URL(checkoutUrl.value).protocol.replace(':', '').toUpperCase() } catch { return '' } })
const canGoUp = computed(() => !!rootUrl.value && browseUrl.value !== rootUrl.value)
const revision = computed(() => revisionMode.value === 'custom' ? form.revision : undefined)
const shortDate = (date: string) => date ? new Date(date).toLocaleDateString(locale.value) : '—'
function resetConnection() { generation++; connectionInfo.value = null; entries.value = []; selectedEntry.value = null; browseUrl.value = ''; rootUrl.value = ''; connectionError.value = ''; connecting.value = false; browsing.value = false }
watch(() => [form.url, revisionMode.value, form.revision], resetConnection)
function validRevision() { return revisionMode.value !== 'custom' || (Number.isInteger(form.revision) && Number(form.revision) >= 0) }
function validUrl() { try { const url = new URL(form.url.trim()); return ['svn:', 'http:', 'https:', 'file:', 'svn+ssh:'].includes(url.protocol) && (url.protocol === 'file:' || !!url.hostname) } catch { return false } }
async function testConnection() {
  if (connecting.value || loading.value) return
  if (!validUrl() || !validRevision()) { connectionError.value = t('desktop.invalidRepository'); return }
  resetConnection()
  const request = ++generation, url = form.url.trim().replace(/\/+$/, '')
  connecting.value = true
  try {
    const info = await svnConnectionInfo(url)
    if (request !== generation) return
    const xml = await svnListRemote(url, revision.value)
    if (request !== generation) return
    entries.value = parseRemoteEntries(xml)
    rootUrl.value = browseUrl.value = url
    connectionInfo.value = info
  } catch (error) { if (request === generation) connectionError.value = String(error) }
  finally { if (request === generation) connecting.value = false }
}
async function browse(url: string) {
  if (browsing.value || connecting.value || loading.value) return
  const request = ++generation
  browsing.value = true; connectionError.value = ''
  try { const xml = await svnListRemote(url, revision.value); if (request === generation) { entries.value = parseRemoteEntries(xml); browseUrl.value = url; selectedEntry.value = null } }
  catch (error) { if (request === generation) connectionError.value = String(error) }
  finally { if (request === generation) browsing.value = false }
}
const enterDirectory = (entry: ReturnType<typeof parseRemoteEntries>[number]) => { if (entry.kind === 'dir') void browse(childRepositoryUrl(browseUrl.value, entry.name)) }
const goUp = () => { if (canGoUp.value) void browse(browseUrl.value.slice(0, browseUrl.value.lastIndexOf('/'))) }
async function selectDirectory() { const selected = await open({ directory: true, multiple: false, title: t('dialog.selectTargetDirectory') }); if (selected) form.path = Array.isArray(selected) ? selected[0] : selected }
async function doCheckout() {
  if (loading.value || connecting.value || browsing.value) return
  if (!validUrl() || !validRevision()) { connectionError.value = t('desktop.invalidRepository'); return }
  if (!/^(\/|[a-zA-Z]:[\\/]|\\\\)/.test(form.path.trim())) { connectionError.value = t('desktop.absolutePath'); return }
  loading.value = true; output.value = ''; outputSucceeded.value = false; connectionError.value = ''
  const path = form.path.trim(), target = checkoutUrl.value, switchAfter = form.switchAfter
  try {
    const result = await svnCheckout(target, path, revision.value, form.depth, form.ignoreExternals)
    if (!result.success) throw new Error(result.error || result.output || t('common.error'))
    output.value = result.output; outputSucceeded.value = true
    if (switchAfter) {
      const loaded = await loadWorkspace(path)
      loading.value = false
      if (loaded) await router.push({ name: 'workspace' })
      else connectionError.value = t('desktop.checkoutOpenFailed')
    } else store.registerWorkspaces([path])
  } catch (error) { output.value = String(error); outputSucceeded.value = false }
  finally { loading.value = false }
}
function resetForm() { if (loading.value) return; form.url = ''; form.path = ''; form.revision = undefined; form.depth = 'infinity'; form.ignoreExternals = false; revisionMode.value = 'head'; output.value = ''; outputSucceeded.value = false; resetConnection() }
onBeforeRouteLeave(() => !loading.value)
onUnmounted(() => { generation++ })
</script>
<style scoped>
.checkout-view{width:100%;max-width:none;margin:0;padding:0;height:100%;display:flex;min-height:0;background:var(--md-sys-color-surface-container-lowest)}.checkout-main{flex:1;min-width:0;display:flex;flex-direction:column}.checkout-form{padding:25px 28px 16px 18px;flex:1;overflow:auto;min-height:0}.checkout-form :deep(.el-form-item){margin-bottom:21px}.checkout-form :deep(.el-form-item__label){font-size:12px;padding-right:16px}.checkout-form :deep(.el-form-item__content){min-width:0;display:block}.url-control{display:flex;gap:8px;width:100%;align-items:center}.url-control>.el-input{min-width:0;flex:1}.url-control>.el-button{flex-shrink:0}.connection-result{display:flex;align-items:center;gap:6px;color:var(--md-sys-color-success);font-size:11px;margin-top:5px}.connection-error{color:var(--md-sys-color-error);font-size:11px;line-height:1.7;white-space:pre-wrap;overflow-wrap:anywhere;margin-top:7px}.remote-browser{border:1px solid var(--md-sys-color-outline-variant);border-radius:7px;overflow:hidden}.remote-breadcrumb{display:flex;align-items:center;gap:8px;padding:2px 8px;border-bottom:1px solid var(--md-sys-color-outline-variant);font-size:10px}.remote-breadcrumb>span{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;color:var(--md-sys-color-on-surface-variant)}.remote-breadcrumb .el-button{flex-shrink:0}.remote-columns,.remote-entry{display:grid;grid-template-columns:minmax(0,1fr) 55px 85px;align-items:center;gap:8px;padding:0 12px}.remote-columns{height:27px;font-size:10px;color:var(--md-sys-color-on-surface-variant);background:var(--md-sys-color-surface-container-low);border-bottom:1px solid var(--md-sys-color-outline-variant)}.remote-rows{height:240px;overflow:auto;padding:4px}.remote-entry{width:100%;height:31px;background:none;border:0;border-radius:5px;text-align:left;font-size:12px;cursor:pointer}.remote-entry>span{display:flex;gap:7px;align-items:center;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.remote-entry small{font-size:10px;color:var(--md-sys-color-on-surface-variant);white-space:nowrap}.remote-entry small:last-child{text-align:right}.remote-entry:hover:enabled{background:var(--md-sys-color-primary-container)}.remote-entry.selected{background:var(--desktop-selected);color:var(--desktop-selected-text)}.remote-entry.selected small{color:var(--desktop-selected-text)}.remote-entry:disabled{opacity:.65;cursor:default}.remote-empty{height:100%;display:flex;flex-direction:column;justify-content:center;align-items:center;color:var(--md-sys-color-on-surface-variant);gap:10px;font-size:12px}.remote-empty>.el-icon{font-size:30px;opacity:.5}.empty-directory{padding:20px;text-align:center;font-size:11px;color:var(--md-sys-color-on-surface-variant)}.field-hint{font-size:10px;color:var(--md-sys-color-on-surface-variant)}.revision-controls{display:flex;gap:10px;align-items:center;flex-wrap:wrap}.revision-controls :deep(.el-radio-button__inner){font-size:11px;padding:8px 10px}.revision-number{width:100px}.depth-select{width:260px;max-width:100%}.checkout-options{display:flex;gap:12px;flex-wrap:wrap;font-size:11px}.checkout-options label{display:flex;align-items:center;gap:6px}.checkout-options input{accent-color:var(--md-sys-color-primary)}.checkout-actions{height:58px;flex-shrink:0;display:flex;align-items:center;justify-content:flex-end;gap:10px;padding:0 28px;border-top:1px solid var(--md-sys-color-outline-variant);background:var(--md-sys-color-surface-container-low)}.checkout-actions>span{margin-right:auto;font-size:10px;color:var(--md-sys-color-on-surface-variant)}.checkout-actions .el-button+.el-button{margin-left:0}.checkout-inspector{width:29%;max-width:350px;min-width:250px;border-left:1px solid var(--md-sys-color-outline-variant);background:var(--md-sys-color-surface-container-low);display:flex;flex-direction:column;overflow:auto}.directory-summary{padding:20px 16px;border-bottom:1px solid var(--md-sys-color-outline-variant)}.directory-summary>small{font-size:11px;color:var(--md-sys-color-on-surface-variant)}.selected-folder{display:flex;align-items:center;gap:10px;margin-top:12px}.selected-folder>span{width:38px;height:38px;display:grid;place-items:center;font-size:20px;border-radius:8px;color:var(--md-sys-color-primary);background:var(--md-sys-color-primary-container);flex-shrink:0}.selected-folder>div{min-width:0}.selected-folder strong{font-size:14px;font-weight:550}.selected-folder small{display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-size:10px;color:var(--md-sys-color-on-surface-variant);margin-top:3px}.checkout-inspector dl{display:grid;grid-template-columns:85px minmax(0,1fr);gap:16px 8px;padding:20px 16px;font-size:11px}.checkout-inspector dt{color:var(--md-sys-color-on-surface-variant)}.checkout-inspector dd{overflow-wrap:anywhere}.checkout-help{padding:20px 16px;border-top:1px solid var(--md-sys-color-outline-variant)}.checkout-help h3{font-size:12px;font-weight:550}.checkout-help p,.checkout-session p{font-size:11px;line-height:1.9;margin-top:8px;color:var(--md-sys-color-on-surface-variant)}.checkout-session{margin:auto 12px 12px;padding:12px;display:flex;gap:10px;align-items:flex-start;background:var(--md-sys-color-surface-container-lowest);border:1px solid var(--md-sys-color-outline-variant);border-radius:7px}.checkout-session>.el-icon{font-size:20px;color:var(--md-sys-color-primary)}.checkout-session strong{font-size:12px;font-weight:550}.output-area{border:1px solid var(--md-sys-color-outline-variant);border-radius:6px;overflow:hidden;margin-left:100px}.output-header{display:flex;align-items:center;justify-content:space-between;padding:8px 10px;font-size:11px;background:var(--md-sys-color-surface-container-low)}.output-area pre{padding:10px;font:11px/1.8 var(--app-font-family-mono);white-space:pre-wrap;overflow-wrap:anywhere}
@media(max-width:1100px){.checkout-inspector{min-width:210px;width:26%}.checkout-form{padding:20px 14px 12px 5px}.checkout-form :deep(.el-form-item__label){width:80px!important;padding-right:10px;font-size:11px}.checkout-form :deep(.el-form-item__content){margin-left:0!important}.checkout-options{font-size:10px}.checkout-actions{padding:0 15px}.output-area{margin-left:80px}.remote-columns,.remote-entry{grid-template-columns:minmax(0,1fr) 38px 65px;padding:0 7px;gap:3px}.checkout-inspector dl{grid-template-columns:70px minmax(0,1fr)}}
@media(max-width:900px){.checkout-inspector{display:none}}
</style>
