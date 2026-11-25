import { invoke } from '@tauri-apps/api/core'
import { ElMessageBox } from 'element-plus/es/components/message-box/index'
import { ElMessage } from 'element-plus/es/components/message/index'
import { h, type VNode } from 'vue'
import i18n from '@/i18n'

export function parseAuthFailure(error: unknown): { server: string; username: string } | null {
  const message = String(error)
  if (!/\bE(?:215004|170001)\b/.test(message)) return null
  const match = /(?:URL|for)\s+'((?:https?|svn)\\?:\/\/[^']+)'/i.exec(message)
  if (!match) return null
  try {
    const url = new URL(match[1].replace(/\\:/g, ':'))
    if (!['https:', 'http:', 'svn:'].includes(url.protocol) || !url.hostname) return null
    // WHATWG URL.origin is "null" for svn://. Scope credentials explicitly to
    // its host and port, treating an omitted port as svnserve's default (3690).
    const server = url.protocol === 'svn:'
      ? `svn://${url.hostname.toLowerCase()}${url.port && url.port !== '3690' ? `:${url.port}` : ''}`
      : url.origin
    return { server, username: decodeURIComponent(url.username) }
  } catch { return null }
}

export function credentialStorageHint(): string {
  return typeof navigator !== 'undefined' && /Mac/i.test(navigator.platform)
    ? 'svnAuth.keychainHint' : 'svnAuth.sessionHint'
}

type CredentialIdentity = { server: string; username: string }
let dialogQueue: Promise<unknown> = Promise.resolve()

const pendingAuth = new Map<string, Promise<CredentialIdentity | false>>()
async function requestCredentials(failure: { server: string; username: string }): Promise<CredentialIdentity | false> {
  const key = `${failure.server}:${failure.username}`
  const existing = pendingAuth.get(key)
  if (existing) return existing
  const operation = dialogQueue.then(async () => {
    const t = i18n.global.t
    let username = failure.username
    let password = ''
    let form: HTMLFormElement | null = null
    try {
      await ElMessageBox.confirm(h('form', { onSubmit: (event: Event) => event.preventDefault(), onVnodeMounted: (vnode: VNode) => { form = vnode.el as HTMLFormElement | null } }, [
        h('label', { class: 'auth-server' }, [t('desktop.server'), h('span', { class: 'el-input' }, [h('span', { class: 'el-input__wrapper' }, [h('input', { class: 'el-input__inner', value: failure.server, readonly: true, 'aria-label': t('desktop.server') })])])]),
        h('label', { class: 'auth-field' }, [t('svnAuth.username'), h('span', { class: 'el-input' }, [h('span', { class: 'el-input__wrapper' }, [h('input', {
          value: username, required: true, class: 'el-input__inner', autocomplete: 'username', 'aria-label': t('svnAuth.username'), readonly: !!failure.username,
          onInput: (event: Event) => { username = (event.target as HTMLInputElement).value },
        })])])]),
        h('label', { class: 'auth-field' }, [t('svnAuth.password'), h('span', { class: 'el-input' }, [h('span', { class: 'el-input__wrapper' }, [h('input', {
          type: 'password', required: true, class: 'el-input__inner', autocomplete: 'current-password', 'aria-label': t('svnAuth.password'),
          onInput: (event: Event) => { password = (event.target as HTMLInputElement).value },
        }), h('button', { type: 'button', class: 'auth-visibility', 'aria-label': t('desktop.showPassword'), 'aria-pressed': 'false', onClick: (event: MouseEvent) => {
          const button = event.currentTarget as HTMLButtonElement
          const input = button.parentElement?.querySelector('input')
          if (input) { input.type = input.type === 'password' ? 'text' : 'password'; button.setAttribute('aria-pressed', String(input.type === 'text')); button.setAttribute('aria-label', t(input.type === 'text' ? 'desktop.hidePassword' : 'desktop.showPassword')) }
        } }, [h('svg', { viewBox: '0 0 24 24', width: '17', height: '17', fill: 'none', stroke: 'currentColor', 'stroke-width': '1.7', 'aria-hidden': 'true' }, [h('path', { d: 'M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7Z' }), h('circle', { cx: '12', cy: '12', r: '3' })])])])])]),
        h('p', { class: 'auth-session-hint' }, t(credentialStorageHint())),
      ]), t('svnAuth.title'), {
        customClass: 'desktop-auth-dialog', confirmButtonText: t('svnAuth.retry'), cancelButtonText: t('common.cancel'),
        closeOnClickModal: false, distinguishCancelAndClose: true,
        beforeClose: (action, _instance, done) => {
          if (action !== 'confirm' || ((form?.reportValidity() ?? true) && username.trim() && password)) done()
        },
      })
      await invoke('configure_svn_credentials', { server: failure.server, username: username.trim(), password })
      return { server: failure.server, username: username.trim() }
    } catch (action) {
      if (action === 'cancel' || action === 'close') return false
      throw action
    } finally { password = '' }
  })
  dialogQueue = operation.catch(() => undefined)
  pendingAuth.set(key, operation)
  try { return await operation } finally { pendingAuth.delete(key) }
}

// Read operations and repeatable updates may retry once after signing in.
// Publishing operations must preflight with svn_connection_info instead.
export async function invokeWithAuthPrompt<T>(command: string, args: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args)
  } catch (error) {
    const failure = parseAuthFailure(error)
    if (!failure) throw error
    const credentials = await requestCredentials(failure)
    if (!credentials) throw error
    const result = await invoke<T>(command, args)
    // Save only a successful login. Storage failure must never repeat an SVN operation.
    try { await invoke('save_svn_credentials', credentials) }
    catch { ElMessage.warning(i18n.global.t('svnAuth.saveFailed')) }
    return result
  }
}
