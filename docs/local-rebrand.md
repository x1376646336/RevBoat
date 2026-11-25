# 版舟 RevBoat 更名记录

本版延续 OrcaSVN v0.6.6 定制版的认证、界面、工作区和历史对比修复。

- 应用包名和可执行文件：RevBoat；窗口标题和关于：版舟 RevBoat。
- 简体中文名称：版舟；英文名称：RevBoat。图标为帆舟与版本节点。
- 新增 public/revboat.svg 与 src-tauri/icons/revboat.svg；npm run icons 可重新生成原生图标。
- 应用标识继续使用 com.orcasvn.desktop。
- 保留所有 orcasvn-* localStorage 键、缓存目录与钥匙串服务名 OrcaSVN SVN credentials:，无需迁移凭据。
- 软件版本继续使用 0.6.6，安装包另以 revboat 标识此次更名。
- 定制版关闭上游更新检查与 WinGet 发布任务，避免将原版作为定制版升级。
- LICENSE 及上游 README（docs/upstream-readme.md）、历史发行记录保留原作者署名。
- 原源码目录继续使用 OrcaSVN，以免现有构建脚本/路径断开；新源码压缩包顶层目录为 RevBoat。

验证：生产构建、现有前端单测、桌面 UI 与 diff/account UI 回归、macOS 包签名及 DMG 校验。
本版未连接用户实际 SVN 服务器，也未在用户钥匙串中写入或删除凭据。
