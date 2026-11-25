const { test } = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const ts = require('typescript')
const vm = require('node:vm')
function compile(file, mocks) {
  const context = { exports: {}, URL, require: name => { assert.ok(name in mocks, name); return mocks[name] } }
  vm.runInNewContext(ts.transpileModule(fs.readFileSync(file, 'utf8'), { compilerOptions: { module: ts.ModuleKind.CommonJS } }).outputText, context)
  return context.exports
}
function failure(url = 'svn://svn.example:3690/project', code = 'E170001') {
  return `SVN 命令执行失败：svn: E170013: Unable to connect to a repository at URL '${url}'\nsvn: ${code}: Can't get username or password`
}
function harness({ run = async () => 'ok', confirm = async () => {} } = {}) {
  const calls = [], dialogs = [], warnings = []
  const invoke = async (command, args) => { calls.push({ command, args }); return run(command, args) }
  const auth = compile('src/api/auth.ts', {
    '@tauri-apps/api/core': { invoke },
    'element-plus/es/components/message/index': { ElMessage: { warning: value => warnings.push(value) } },
    'element-plus/es/components/message-box/index': { ElMessageBox: { confirm: async (...args) => { dialogs.push(args); return confirm(...args) } } },
    vue: { h: (tag, props, children) => ({ tag, props: children === undefined ? {} : props, children: children === undefined ? props : children }) },
    '@/i18n': { default: { global: { t: key => key } } },
  })
  const api = compile('src/api/svn.ts', { '@tauri-apps/api/core': { invoke }, './auth': auth })
  return { ...auth, api, calls, dialogs, warnings }
}
function fill(message, username = 'demo', password = 'fictional-secret') {
  message.children[1].children[1].children[0].children[0].children.onInput({ target: { value: username } })
  const input = message.children[2].children[1].children[0].children[0].children
  assert.equal(input.type, 'password')
  input.onInput({ target: { value: password } })
}

test('parses native SVN and HTTP(S), canonicalizing ports without an opaque null origin', () => {
  const h = harness()
  for (const [url, server, username] of [
    ['svn://SVN.example/project', 'svn://svn.example', ''],
    ['svn://svn.example:3690/project', 'svn://svn.example', ''],
    ['svn://demo@svn.example:3691/project', 'svn://svn.example:3691', 'demo'],
    ['svn://[::1]:3690/project', 'svn://[::1]', ''],
    ['svn://[::1]:3691/project', 'svn://[::1]:3691', ''],
    ['svn\\://demo@svn.example/project', 'svn://svn.example', 'demo'],
    ['https://demo@svn.example:443/project', 'https://svn.example', 'demo'],
    ['http://svn.example:8080/project', 'http://svn.example:8080', ''],
  ]) {
    const result = h.parseAuthFailure(failure(url))
    assert.equal(result?.server, server, url)
    assert.equal(result?.username, username, url)
  }
  for (const error of [failure('svn+ssh://svn.example/project'), failure('file:///repo'), failure('svn:///repo'), failure(undefined, 'E170013'), 'svn: E170001: no server URL']) {
    assert.equal(h.parseAuthFailure(error), null)
  }
})

test('native checkout prompts and checks authentication before checking out exactly once', async () => {
  let ready = false
  const h = harness({ run: async (command, args) => {
    if (command === 'configure_svn_credentials') {
      assert.equal(args.server, 'svn://svn.example')
      assert.equal(args.username, 'demo')
      assert.equal(args.password, 'fictional-secret')
      ready = true
    } else if (command === 'svn_connection_info' && !ready) throw failure()
    return { success: true, output: 'Checked out revision 1.' }
  }, confirm: message => fill(message) })
  assert.equal((await h.api.svnCheckout('svn://svn.example/project', '/wc')).success, true)
  assert.deepEqual(h.calls.map(x => x.command), ['svn_connection_info', 'configure_svn_credentials', 'svn_connection_info', 'save_svn_credentials', 'svn_checkout'])
  assert.equal(h.dialogs.length, 1)
})

test('existing working copies prompt for update, remote revision and history', async () => {
  for (const action of [h => h.api.svnUpdate('/wc'), h => h.api.svnRemoteInfo('/wc'), h => h.api.svnLog('/wc')]) {
    let authenticated = false
    const h = harness({ run: async command => {
      if (command === 'configure_svn_credentials') { authenticated = true; return }
      if (!authenticated) throw failure()
      return 'ok'
    }, confirm: message => fill(message) })
    assert.equal(await action(h), 'ok')
    assert.equal(h.dialogs.length, 1)
    assert.equal(h.calls.length, 4)
  }
})

test('cancelling or closing authentication prevents checkout and publishing changes', async () => {
  for (const action of ['cancel', 'close']) {
    const error = failure()
    const h = harness({ run: async () => { throw error }, confirm: async () => { throw action } })
    for (const operation of [() => h.api.svnCheckout('svn://svn.example/repo', '/wc'), () => h.api.svnCommit('/wc', 'message'), () => h.api.svnSwitch('/wc', 'svn://svn.example/branch'), () => h.api.svnMerge('/wc', 'svn://svn.example/branch', 1, 2)]) {
      await assert.rejects(operation(), value => value === error)
    }
    assert.ok(h.calls.every(x => x.command === 'svn_connection_info'))
  }
})

test('rejected credentials stop after one retry; the next user attempt can prompt again', async () => {
  const h = harness({ run: async command => { if (command !== 'configure_svn_credentials') throw failure() }, confirm: message => fill(message) })
  await assert.rejects(h.api.svnUpdate('/wc'))
  assert.equal(h.dialogs.length, 1)
  assert.equal(h.calls.filter(x => x.command === 'svn_update').length, 2)
  await assert.rejects(h.api.svnUpdate('/wc'))
  assert.equal(h.dialogs.length, 2)
})

test('a failed commit is never automatically published twice', async () => {
  const h = harness({ run: async command => { if (command === 'svn_commit') throw failure(); return {} }, confirm: message => fill(message) })
  await assert.rejects(h.api.svnCommit('/wc', 'message'))
  assert.deepEqual(h.calls.map(x => x.command), ['svn_connection_info', 'svn_commit'])
  assert.equal(h.dialogs.length, 0)
})

test('concurrent requests to one server share a single credential dialog', async () => {
  let accept, ready = false
  const pending = new Promise(resolve => { accept = resolve })
  const h = harness({ run: async (command, args) => {
    if (command === 'configure_svn_credentials') { ready = true; return }
    if (!ready) throw failure()
    return args.id
  }, confirm: async message => { fill(message); await pending } })
  const requests = ['a', 'b'].map(id => h.invokeWithAuthPrompt('svn_log', { id }))
  await new Promise(resolve => setImmediate(resolve))
  assert.equal(h.dialogs.length, 1)
  accept()
  assert.deepEqual(await Promise.all(requests), ['a', 'b'])
  assert.equal(h.calls.filter(x => x.command === 'configure_svn_credentials').length, 1)
})

test('non-authentication failures do not request credentials or retry', async () => {
  const error = failure(undefined, 'E170013')
  const h = harness({ run: async () => { throw error } })
  await assert.rejects(h.api.svnUpdate('/wc'), value => value === error)
  assert.equal(h.dialogs.length, 0)
  assert.equal(h.calls.length, 1)
})


test('successful GUI login is persisted only after the authenticated operation succeeds', async () => {
  let ready = false
  const h = harness({ run: async (command, args) => {
    if (command === 'configure_svn_credentials') { ready = true; return }
    if (!ready) throw failure()
    if (command === 'save_svn_credentials') {
      assert.equal(args.server, 'svn://svn.example')
      assert.equal(args.username, 'demo')
      assert.equal('password' in args, false)
    }
    return 'result'
  }, confirm: message => fill(message) })
  assert.equal(await h.api.svnUpdate('/wc'), 'result')
  assert.deepEqual(h.calls.map(x => x.command), ['svn_update', 'configure_svn_credentials', 'svn_update', 'save_svn_credentials'])
  await h.api.svnUpdate('/wc')
  assert.equal(h.dialogs.length, 1)
})

test('Keychain save failure keeps the successful result and never repeats the operation', async () => {
  let ready = false
  const h = harness({ run: async command => {
    if (command === 'configure_svn_credentials') { ready = true; return }
    if (command === 'save_svn_credentials') throw new Error('vault unavailable')
    if (!ready) throw failure()
    return 'updated'
  }, confirm: message => fill(message) })
  assert.equal(await h.api.svnUpdate('/wc'), 'updated')
  assert.equal(h.calls.filter(c => c.command === 'svn_update').length, 2)
  assert.deepEqual(h.warnings, ['svnAuth.saveFailed'])
})

test('invalid credentials are never saved', async () => {
  const h = harness({ run: async command => {
    if (command !== 'configure_svn_credentials') throw failure()
  }, confirm: message => fill(message) })
  await assert.rejects(h.api.svnUpdate('/wc'))
  assert.equal(h.calls.some(c => c.command === 'save_svn_credentials'), false)
})
