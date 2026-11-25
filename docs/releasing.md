# 正式发布

## 工作流区别

| 工作流 | 触发条件 | 作用 |
| --- | --- | --- |
| `ci.yml` | 推送 `main` 或提交 Pull Request | Linux 构建检查与测试、Windows SVN Unicode 回归；不生成发行安装包 |
| `tauri-build.yml` | 推送正式版本标签 `vX.Y.Z` | Windows、macOS、Linux 检查、打包和正式发布 |
| `prerelease.yml` | 推送 `vX.Y.Z-rc.N` 等预发布标签 | 三平台检查、打包并发布预发布版本 |

CI 的红色失败只说明对应检查未通过。普通代码推送不会直接生成三个系统的发行安装包。

正式标签使用 `vX.Y.Z`；预发布标签使用 `vX.Y.Z-alpha.N`、`beta.N` 或 `rc.N`，见[预发布流程](prerelease.md)。不要移动已公开版本的标签。

**当前项目版本为 `0.6.6`，对应标签为 `v0.6.6`。** `v20261125` 这样的纯日期标签无法通过版本校验；日期可以写在 Release 标题或说明中。如果已经误建并公开了日期标签，保留原记录，使用匹配软件版本且尚未使用的新标签触发构建。仅点击旧任务的 Re-run 不会更换标签。

## 发布前

将 `package.json`、`package-lock.json` 顶层及根包版本、`src-tauri/Cargo.toml`、`src-tauri/Cargo.lock` 的 RevBoat 包版本、`src-tauri/tauri.conf.json` 统一为正式版本。更新用户文档及 `docs/releases/vX.Y.Z.md`；发布流水线从该文件读取公开更新说明。

在项目根目录运行：

```bash
RELEASE_TAG=v0.6.6 node scripts/validate-release.cjs
npm run check
```

版本校验拒绝预发布标识、前导零、缺失或不一致的版本。截图日期、历史性能记录保留原始来源；新基准应记录提交、环境、测量边界和可重跑方法，不能把原生数据请求耗时当作桌面启动耗时。

确认发布范围并提交、推送到 `main` 后创建标签：

```bash
git tag -a v0.6.6 -m "RevBoat v0.6.6"
git push origin v0.6.6
```

上面的版本号适用于本次发布；后续发布先同步更新所有版本文件和发布说明，再使用对应的新标签。推送标签会自动创建草稿，等待三个系统打包成功后公开；无需先在 GitHub 手动公开 Release。

## 自动流程

`.github/workflows/tauri-build.yml` 只处理正式标签：

1. 校验所有版本文件和发布说明，创建或复用 GitHub Release 草稿；已公开的同名版本拒绝重新发布。
2. Windows、macOS、Linux 安装 SVN 测试工具，运行 `npm run check`，打包并将安装包上传至同一个草稿。
3. 所有平台成功后才公开为正式版并标记 Latest。
4. 本定制版不向上游 OrcaSVN 的 WinGet 条目发布，也不需要 `WINGET_TOKEN`。

Windows 固定使用 SlikSVN 1.14.5，核验 svn 与 svnadmin 后加入 PATH；正式版本生成 NSIS EXE 和 MSI。macOS 使用 `macos-latest` 的原生架构，当前为 Apple Silicon；Linux 使用 Ubuntu x64。

任一平台失败时，Release 保持草稿，Latest 不变。在 Actions 中重跑失败任务；流程代码修改不会应用到旧标签上的重跑。需要新的源码或流程修复时使用新的版本标签，不强制移动已发布标签。

## 发布后验证

检查 Actions 三平台检查、打包和公开步骤，以及 Release 的版本、说明、Latest 标记和实际安装包列表。本地检查和 CI 构建不能替代在各操作系统上安装、启动和真实仓库操作的验收。
