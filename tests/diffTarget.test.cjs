const {test}=require('node:test')
const assert=require('node:assert/strict')
const ts=require('typescript'),vm=require('node:vm'),fs=require('node:fs')
const context={exports:{}}
vm.runInNewContext(ts.transpileModule(fs.readFileSync('src/utils/diffTarget.ts','utf8'),{compilerOptions:{module:ts.ModuleKind.CommonJS}}).outputText,context)
const {repositoryDiffTarget,isRepositoryDiffTarget}=context.exports
test('history targets preserve special filenames and use the preceding peg for deletions',()=>{
 const path='/trunk/文档 #100%?@v1.txt'
 const target=repositoryDiffTarget('svn://example/repo/',path,5,'M')
 assert.equal(target,'svn://example/repo/trunk/%E6%96%87%E6%A1%A3%20%23100%25%3F%40v1.txt@5')
 assert.equal(repositoryDiffTarget('https://example/repo',path,5,'D'),target.replace('svn://','https://').replace('@5','@4'))
 assert.equal(repositoryDiffTarget('file:///repo','/new.txt',1,'A'),'file:///repo/new.txt@1')
 assert.equal(isRepositoryDiffTarget(target),true)
 assert.equal(isRepositoryDiffTarget('src/file.txt'),false)
})
