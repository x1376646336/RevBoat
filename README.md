# 版舟 RevBoat

SVN 桌面客户端。每一次改动，都有迹可循。

基于 OrcaSVN v0.6.6 的本地定制版本，保留上游 MIT 许可证及版权声明。
原项目：https://github.com/wustites/OrcaSVN
原版说明见 [上游 README](docs/upstream-readme.md)。

## 本版功能

- 蓝灰色工作台：本地变更、提交历史、检出仓库与文件对比。
- 图形化账号密码输入，成功认证后保存在 macOS 钥匙串。
- 设置 → SVN 账号 → 修改账号 / 密码。
- 工作区目录直接选择，历史版本文件对比与特殊字符路径修复。
- 使用 RevBoat 应用名称，沿用原有工作区设置和钥匙串存储标识。
- 定制版通过新安装包更新，不提示安装上游 OrcaSVN 版本。

## 当前 macOS 安装包

适用于 Apple Silicon、macOS 26 及以上，包含 SVN 运行时。
退出旧应用后，将 RevBoat.app 拖入 Applications，再启动 RevBoat。
旧 OrcaSVN.app 不会被自动删除；请勿同时运行两个版本。
安装包为本机临时签名，未经 Apple 公证。

## 开发与构建

```sh
npm ci
npm run dev
npm run build
npm run test:unit
cargo test --manifest-path src-tauri/Cargo.toml
npm run icons
npm run build:with-svn -- /absolute/path/to/svn-runtime --bundles app
```

运行时目录须含 svn 可执行文件、依赖及 licenses 目录。
本次仅验证 macOS 安装包；保留的 Windows/Linux 构建脚本尚未验证。
更名兼容性说明见 [本地更名记录](docs/local-rebrand.md)。
