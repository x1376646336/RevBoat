// Production front end with isolated IPC fixtures. Never uses real accounts/repositories.
const {chromium}=require('playwright')
const assert=require('node:assert/strict')
const path=require('node:path')
const {fixture}=require('./check-desktop-ui.cjs')
async function main(){
 const browser=await chromium.launch({channel:'chrome',headless:true})
 try {
  const page=await browser.newPage({viewport:{width:1440,height:900},deviceScaleFactor:2})
  const errors=[];page.on('pageerror',e=>errors.push(e.message))
  await fixture(page)
  await page.addInitScript(()=>{
   const original=window.__TAURI_INTERNALS__.invoke
   window.__diffResultMode='normal';window.__accountName='old-account';window.__accountChanges=0
   window.__TAURI_INTERNALS__.invoke=async(command,args={})=>{
    if(command==='get_svn_account')return {server:'https://svn.example.com',username:window.__accountName,has_credentials:true,explicit_username:false}
    if(command==='change_svn_account'){
     window.__accountChanges++
     if(args.password==='wrong')throw 'SVN: E170001: Authentication failed'
     if(args.username!=='new-account'||args.password!=='synthetic-password')throw 'Unexpected test credentials'
     window.__accountName=args.username;return true
    }
    if(command==='svn_diff'){
     const result=await original(command,args)
     if(window.__diffResultMode==='error')throw 'SVN: E160013: requested historical path does not exist'
     if(window.__diffResultMode==='empty')return {...result,diff:''}
     if(window.__diffResultMode==='binary')return {...result,diff:'Index: example.docx\nCannot display: file marked as a binary type.\nsvn:mime-type = application/octet-stream\n'}
     return result
    }
    const result=await original(command,args)
    if(command==='svn_log')return result.map(entry=>({...entry,changed_paths:[{path:'/trunk/文档 #100%?@v1.txt',action:'M'},{path:'/trunk/deleted.txt',action:'D'}]}))
    return result
   }
  })
  await page.goto('http://127.0.0.1:1424/#/log')
  await page.locator('.history-row').nth(1).click()
  await page.locator('.history-preview .diff-code').first().waitFor()
  let call=await page.evaluate(()=>window.__desktopCalls.filter(c=>c.command==='svn_diff').at(-1))
  assert.match(call.args.file,/%23.*%25%3F%40v1.txt@54$/)
  assert.equal(call.args.oldRev,54)
  await page.getByRole('button',{name:'查看完整差异',exact:true}).click()
  await page.waitForURL('**/#/diff?**')
  await page.locator('.diff-main .diff-code').first().waitFor()
  assert.equal(await page.locator('.run-diff').isVisible(),true)
  assert.equal(await page.locator('.mode-switch input').nth(0).isDisabled(),true)
  await page.locator('.mode-switch .el-radio-button').nth(1).click()
  await page.locator('.revision-inputs input').nth(0).fill('1')
  await page.locator('.revision-inputs input').nth(1).fill('2')
  await page.locator('.run-diff').click()
  await page.waitForFunction(()=>{const a=window.__desktopCalls.filter(c=>c.command==='svn_diff').at(-1)?.args;return a?.oldRev===1&&a?.newRev===2})
  await page.locator('.mode-switch .el-radio-button').nth(2).click()
  await page.waitForFunction(()=>{const a=window.__desktopCalls.filter(c=>c.command==='svn_diff').at(-1)?.args;return a?.oldRev===54&&a?.newRev===undefined})
  await page.locator('.sidebar-file').nth(1).click()
  await page.waitForFunction(()=>window.__desktopCalls.filter(c=>c.command==='svn_diff').at(-1)?.args.file.endsWith('deleted.txt@53'))
  await page.waitForFunction(()=>[...document.querySelectorAll('.diff-header .el-tag')].every(el=>getComputedStyle(el).opacity==='1'))
  await page.screenshot({path:path.resolve(__dirname,'../../RevBoat-新版截图/06-历史文件对比.png'),animations:'disabled'})
  await page.evaluate(()=>window.__diffResultMode='empty');await page.locator('.run-diff').click();await page.locator('.empty-result').waitFor()
  await page.evaluate(()=>window.__diffResultMode='error');await page.locator('.run-diff').click();await page.locator('.diff-error').waitFor();assert.match(await page.locator('.diff-error').textContent(),/E160013/)
  await page.evaluate(()=>window.__diffResultMode='binary');await page.locator('.run-diff').click();await page.locator('.binary-notice').waitFor()
  await page.getByRole('button',{name:'设置',exact:true}).click()
  await page.locator('.current-account').filter({hasText:'old-account'}).waitFor()
  await page.locator('.edit-account').click()
  await page.locator('.account-editor input[autocomplete="username"]').fill('new-account')
  await page.locator('.account-editor input[autocomplete="current-password"]').fill('wrong')
  await page.locator('.save-account').click();await page.locator('.account-error').waitFor()
  assert.equal(await page.locator('.current-account').textContent(),'old-account')
  assert.equal(await page.evaluate(()=>window.__accountName),'old-account')
  assert.equal(await page.locator('.account-editor input[autocomplete="current-password"]').inputValue(),'')
  await page.locator('.account-editor input[autocomplete="current-password"]').fill('synthetic-password')
  await page.locator('.save-account').click();await page.locator('.account-notice').waitFor()
  assert.equal(await page.locator('.current-account').textContent(),'new-account')
  assert.equal(await page.evaluate(()=>window.__accountName),'new-account')
  assert.equal(await page.evaluate(()=>Object.values(localStorage).concat(Object.values(sessionStorage)).some(v=>v.includes('synthetic-password'))),false)
  await page.screenshot({path:path.resolve(__dirname,'../../RevBoat-新版截图/07-账号设置.png'),animations:'disabled'})
  assert.deepEqual(errors,[])
  console.log('History diff and account UI passed: encoded targets, deletion peg, mode changes, visible compare action, empty/error/binary results, rejected account preserves old login, verified account updates, no browser password storage.')
 }finally{await browser.close()}
}
main().catch(e=>{console.error(e);process.exitCode=1})
