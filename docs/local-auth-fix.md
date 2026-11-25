> 更新：本文的“仅会话保存”描述对应最初认证修复。UI fix1 已加入 macOS 钥匙串持久保存，详见 `local-ui-redesign.md` 的 fix1 章节。

# 本地 SVN 图形界面认证修复

本地修改版，以 wustites/OrcaSVN 的 v0.6.6 官方源码为基础。未向上游提交，也不是上游官方新版本。

## 基线比较

核对日期：2026-11-25。下表时间为 America/Los_Angeles（UTC−08:00）。

| 项目 | 最新正式版 | 发布时间 | 默认分支提交 |
| --- | --- | --- | --- |
| firzen/StoneSVN | v1.3.1 | 2026-11-24 08:03:08 | d83bf131c65dc543b26c35163e43f1d38d33eb91 |
| wustites/OrcaSVN | v0.6.6 | 2026-11-24 10:33:06 | 7b7e53ed34a1011a3491889c4de65234ae069fb4 |

StoneSVN 是 OrcaSVN 的 fork。两者版本号独立，不能以 1.3.1 大于 0.6.6 判断代码新旧。按用户要求选用发布时间和默认分支提交时间均较新的 OrcaSVN v0.6.6。其默认分支与本次正式版对应同一提交。

来源：
- https://api.github.com/repos/firzen/StoneSVN/releases/latest
- https://api.github.com/repos/wustites/OrcaSVN/releases/latest
- https://api.github.com/repos/firzen/StoneSVN
- https://api.github.com/repos/wustites/OrcaSVN/commits/main
- https://github.com/wustites/OrcaSVN/releases/tag/v0.6.6

Git 智能传输多次连接失败，因此完整源码通过 GitHub 官方 tag 归档下载，并建立本地基线提交供查看差异。上表中的上游提交号与本地导入提交号不同。

## 行为

- SVN 返回 E170001 或 E215004 且错误中包含可识别的 `svn://`、`http://` 或 `https://` 仓库地址时，弹出账号、密码输入框。
- 适用于新检出、已有工作副本更新、远端版本和日志，以及远端差异和追溯。
- 登录成功后继续原操作。取消停止本次操作；错误密码只重试一次，再次点击原操作可重新输入。
- 提交、检出、切换、合并先做只读连接检查，避免自动重复发布操作。
- 凭据仅保存于后端本次进程内存，按协议、主机、端口及 URL 中指定的账号匹配；`svn://host` 与 `svn://host:3690` 等价。
- 密码通过子进程标准输入传递，不放入命令行参数、前端持久化设置或 SVN 认证缓存。
- SSH 隧道 (`svn+ssh://`) 保持原行为，本次不实现 SSH 密码交互。
- 沿用 OrcaSVN v0.6.6 已有的检出成功/失败状态修复，另外处理显式 `success: false` 的返回结果。
- macOS 本地包包含可移动的 SVN 运行环境，默认自动从应用资源目录发现，无需终端配置 PATH。

认证弹窗、标准输入传递及运行环境打包脚本参考/改编了 MIT 许可的 firzen/StoneSVN v1.3.1，并补齐原生 SVN 协议与端口隔离、OrcaSVN 接口适配及测试。保留项目 MIT 许可证。

## 开发与验证

```sh
npm ci
npm run check
```

完整后端测试需要 `svn`、`svnadmin` 和 `svnserve`。以下环境变量使新增认证集成测试在缺少工具时明确失败：

```sh
ORCASVN_REQUIRE_AUTH_INTEGRATION=1 npm run check
```

图形界面回归使用虚构服务器及 IPC 返回值，不访问公司仓库：

```sh
npx playwright install chromium
npm run preview -- --host 127.0.0.1 --port 1424
# 另一终端
npm run test:auth-ui
```

如果使用已安装的 Chrome，可给测试命令添加 `PLAYWRIGHT_CHANNEL=chrome`。可用 `AUTH_UI_LANGUAGE=zh-CN` 只跑中文场景。

macOS 打包需要 Rust、Xcode Command Line Tools，以及通过 Homebrew 安装的 Subversion。以下命令将运行环境和许可证一起纳入应用：

```sh
python3 scripts/stage-macos-svn.py /absolute/path/to/new-svn-runtime
APPLE_SIGNING_IDENTITY=- npm run build:with-svn -- /absolute/path/to/new-svn-runtime --bundles app
```

该打包方式使用本地 ad-hoc 签名，未进行 Apple Developer ID 公证。Windows/Linux 未在本机执行原生打包验收。
