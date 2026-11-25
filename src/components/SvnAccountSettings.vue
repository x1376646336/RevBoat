<template>
  <section class="account-settings">
    <h3 class="section-title"><el-icon><User /></el-icon>{{ $t('accountUi.title') }}</h3>
    <p v-if="!supported" class="account-hint">{{ $t('accountUi.selectWorkspace') }}</p>
    <template v-else>
      <div class="account-summary">
        <span>{{ $t('desktop.server') }}</span><strong class="account-server">{{ info?.server || serverLabel }}</strong>
        <span>{{ $t('accountUi.currentAccount') }}</span><strong class="current-account">{{ loading ? $t('common.loading') : info?.username || $t('accountUi.notConfigured') }}</strong>
      </div>
      <p class="account-hint">{{ $t('accountUi.scopeHint') }}</p>
      <el-button v-if="!editing" class="edit-account" :disabled="loading" @click="startEditing">{{ $t('accountUi.edit') }}</el-button>
      <div v-else class="account-editor">
        <label>{{ $t('svnAuth.username') }}<el-input v-model="username" :readonly="info?.explicit_username" :disabled="saving" autocomplete="username" :aria-label="$t('svnAuth.username')" /></label>
        <p v-if="info?.explicit_username" class="account-hint">{{ $t('accountUi.pinnedAccount') }}</p>
        <label>{{ $t('svnAuth.password') }}<el-input v-model="password" type="password" show-password :disabled="saving" autocomplete="current-password" :aria-label="$t('svnAuth.password')" @keyup.enter="save" /></label>
        <p class="account-hint">{{ $t(credentialStorageHint()) }}</p>
        <div class="account-actions"><el-button :disabled="saving" @click="cancel">{{ $t('common.cancel') }}</el-button><el-button class="save-account" type="primary" :loading="saving" :disabled="!username.trim() || !password" @click="save">{{ $t('accountUi.verifySave') }}</el-button></div>
      </div>
      <p v-if="error" class="account-error" role="alert">{{ error }}</p>
      <p v-if="notice" class="account-notice" role="status">{{ notice }}</p>
    </template>
  </section>
</template>
<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from 'vue-i18n'
import { credentialStorageHint } from '@/api/auth'
import { useWorkspaceStore } from '@/stores/workspace'
interface AccountInfo { server: string; username: string; explicit_username: boolean; has_credentials: boolean }
const store = useWorkspaceStore()
const { t } = useI18n()
const target = computed(() => store.svnInfo?.url || '')
const supported = computed(() => /^(?:https?|svn):\/\//i.test(target.value))
const serverLabel = computed(() => { try { const url = new URL(target.value); return `${url.protocol}//${url.host}` } catch { return '' } })
const info = ref<AccountInfo | null>(null)
const loading = ref(false)
const editing = ref(false)
const saving = ref(false)
const username = ref('')
const password = ref('')
const error = ref('')
const notice = ref('')
let generation = 0
watch(target, async value => {
  const current = ++generation
  info.value = null; editing.value = false; username.value = ''; password.value = ''; error.value = ''; notice.value = ''
  loading.value = supported.value
  if (!supported.value) return
  try { const result = await invoke<AccountInfo>('get_svn_account', { target: value }); if (current === generation) info.value = result }
  catch (e) { if (current === generation) error.value = String(e) }
  finally { if (current === generation) loading.value = false }
}, { immediate: true })
function startEditing() { username.value = info.value?.username || ''; password.value = ''; error.value = ''; notice.value = ''; editing.value = true }
function cancel() { editing.value = false; password.value = ''; error.value = '' }
async function save() {
  if (saving.value || !username.value.trim() || !password.value || !supported.value) return
  const current = generation
  saving.value = true; error.value = ''; notice.value = ''
  try {
    const saved = await invoke<boolean>('change_svn_account', { target: target.value, username: username.value.trim(), password: password.value })
    if (current !== generation) return
    info.value = { server: info.value?.server || serverLabel.value, username: username.value.trim(), explicit_username: info.value?.explicit_username || false, has_credentials: true }
    editing.value = false
    notice.value = !saved && credentialStorageHint() === 'svnAuth.keychainHint' ? t('svnAuth.saveFailed') : t('accountUi.updated')
  } catch (e) { if (current === generation) error.value = String(e) }
  finally { password.value = ''; saving.value = false }
}
onBeforeUnmount(() => { generation++; password.value = '' })
</script>
<style scoped>
.account-settings { margin-bottom: 20px; }
.section-title { display:flex;align-items:center;gap:8px;font-size:15px;margin:0 0 18px; }
.account-summary { display:grid;grid-template-columns:110px minmax(0,1fr);gap:10px;font-size:13px; }
.account-summary>span,.account-hint { color:var(--el-text-color-secondary); }
.account-summary strong { font-weight:500;overflow-wrap:anywhere; }
.account-hint { font-size:12px;line-height:1.7;margin:12px 0; }
.account-editor { padding:14px;border:1px solid var(--md-sys-color-outline-variant);border-radius:7px;background:var(--md-sys-color-surface-container-low); }
.account-editor label { display:grid;grid-template-columns:90px minmax(0,1fr);align-items:center;margin-bottom:12px;font-size:13px; }
.account-actions { display:flex;justify-content:flex-end;gap:8px; }
.account-error,.account-notice { font-size:12px;white-space:pre-wrap;overflow-wrap:anywhere;line-height:1.7; }
.account-error { color:var(--el-color-danger); }.account-notice { color:var(--el-color-success); }
</style>
