// Run against the production preview at port 1424, with Playwright available.
// All repository responses and credentials are fictional.
const assert = require('node:assert/strict')
const { chromium } = require('playwright')
const path = require('node:path')
const baseUrl = process.env.AUTH_UI_BASE_URL || 'http://127.0.0.1:1424'
async function main() {
  const browser = await chromium.launch({ headless: true, ...(process.env.PLAYWRIGHT_CHANNEL ? { channel: process.env.PLAYWRIGHT_CHANNEL } : {}) })
  try {
    for (const language of ['en-US', 'zh-CN', 'zh-TW', 'ja-JP', 'ko-KR']) {
      if (process.env.AUTH_UI_LANGUAGE && process.env.AUTH_UI_LANGUAGE !== language) continue
      for (const route of ['log', 'checkout']) {
        for (const outcome of ['success', 'cancel', 'rejected']) {
          const context = await browser.newContext({ viewport: { width: 1200, height: 800 } })
          try {
            const page = await context.newPage()
            const errors = []
            page.on('pageerror', error => errors.push(error.message))
            await page.addInitScript(({ language, outcome }) => {
              localStorage.setItem('orcasvn-last-workspace', '/workspaces/auth-demo')
              localStorage.setItem('orcasvn-recent-workspaces', '["/workspaces/auth-demo"]')
              localStorage.setItem('orcasvn-settings', JSON.stringify({ language }))
              window.__authTest = { probes: 0, configurations: 0, checkouts: 0, saves: 0 }
              let authenticated = false
              window.__TAURI_INTERNALS__ = { invoke: async (command, args) => {
                const state = window.__authTest
                if (command === 'configure_svn_executable') return '1.14.5'
                if (command === 'svn_info' || command === 'svn_remote_info') return { path: '/workspaces/auth-demo', url: 'svn://svn.example.com/project', repository_root: 'svn://svn.example.com/project', revision: 1, node_kind: 'dir', schedule: 'normal' }
                if (command === 'svn_local_revision') return 1
                if (command === 'svn_status') return []
                if (command === 'save_svn_credentials') { if (!authenticated) throw new Error('Must not persist failed login'); state.saves++; return true }
                if (command === 'configure_svn_credentials') {
                  if (args.server !== 'svn://svn.example.com' || args.username !== 'demo' || args.password !== 'fictional-password') throw new Error('Unexpected credentials')
                  state.configurations++
                  authenticated = outcome === 'success'
                  return
                }
                if (command === 'svn_log' || command === 'svn_connection_info') {
                  state.probes++
                  if (authenticated) return command === 'svn_log' ? [] : {}
                  throw "SVN 命令执行失败：svn: E170013: Unable to connect to a repository at URL 'svn://svn.example.com:3690/project'\nsvn: E170001: Can't get username or password"
                }
                if (command === 'svn_checkout') { state.checkouts++; return { success: true, output: 'Checked out revision 1.', error: null, conflicts: [] } }
                return null
              } }
            }, { language, outcome })
            await page.goto(`${baseUrl}/#/${route}`)
            if (route === 'checkout') {
              const fields = page.locator('.checkout-form input')
              await fields.nth(0).fill('svn://svn.example.com:3690/project')
              await fields.nth(1).fill('/workspaces/new-copy')
              await page.locator('.form-actions .el-button--primary').click()
            }
            const dialog = page.locator('.el-message-box')
            await dialog.waitFor()
            const username = dialog.locator('input[autocomplete="username"]')
            assert.equal(await username.inputValue(), '')
            assert.equal(await username.getAttribute('readonly'), null)
            assert.match(await dialog.locator('.auth-server input').inputValue(), /svn:\/\/svn.example.com/)
            if (language === 'zh-CN' && route === 'checkout' && outcome === 'success' && process.env.AUTH_UI_SCREENSHOT) {
              await page.waitForTimeout(300)
              await page.screenshot({ animations: 'disabled', path: path.resolve(process.env.AUTH_UI_SCREENSHOT) })
            }
            if (outcome === 'cancel') {
              await dialog.locator('.el-button').first().click()
            } else {
              await dialog.locator('.el-button--primary').click()
              assert.equal(await dialog.isVisible(), true)
              assert.equal(await page.evaluate(() => window.__authTest.configurations), 0)
              await username.fill('demo')
              await dialog.locator('input[type="password"]').fill('fictional-password')
              await dialog.locator('.el-button--primary').click()
            }
            await dialog.waitFor({ state: 'hidden' })
            await page.waitForFunction(expected => window.__authTest.probes === expected, outcome === 'cancel' ? 1 : 2)
            const state = await page.evaluate(() => window.__authTest)
            assert.equal(state.configurations, outcome === 'cancel' ? 0 : 1)
            assert.equal(state.saves, outcome === 'success' ? 1 : 0)
            assert.equal(state.checkouts, route === 'checkout' && outcome === 'success' ? 1 : 0)
            if (route === 'checkout') {
              if (outcome === 'success') await page.waitForURL('**/#/')
              else {
                const tag = page.locator('.output-header .el-tag')
                await tag.waitFor()
                assert.match(await tag.getAttribute('class'), /el-tag--danger/)
              }
            }
            assert.equal(await page.evaluate(() => Object.values(localStorage).some(value => value.includes('fictional-password'))), false)
            assert.deepEqual(errors, [])
            console.log(`${language}: ${route} ${outcome} passed`)
          } finally { await context.close() }
        }
      }
    }
  } finally { await browser.close() }
}
main().catch(error => { console.error(error); process.exitCode = 1 })
