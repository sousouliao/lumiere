# macOS 检查更新与自动更新方案调研（2026-09-09）

## 结论

Lumiere 应将 macOS 更新分为两个阶段：

1. **近期先实现“检查更新 → 打开下载页”**。最小实现可直接读取 GitHub latest
   release，但这只能作为 best-effort 能力；若中国大陆可用性是产品要求，应让应用优先
   读取自有域名上的静态 manifest，再把 GitHub Releases 保留为手动下载入口或备用入口。
   这一阶段不自动下载或替换应用，可以在当前 ad-hoc 签名、未公证、只发布 DMG 的条件下
   安全交付。
2. **获得稳定 Developer ID 签名并建立公证流水线后，使用现有 `electron-updater`
   扩展 macOS 自动更新**。发布时保留 DMG 给首次安装，另外生成每个架构的 ZIP、
   blockmap 和唯一的 `latest-mac.yml` 供应用内更新。若届时仍要求降低大陆终端对 GitHub
   运行时链路的依赖，应使用 `generic` provider 将 metadata 和 ZIP 托管到自有域名/CDN；
   GitHub provider 则适合接受 GitHub 可达性为 best-effort 的更小发布方案。

`electron-updater` 是当前项目的最小充分长期方案：Lumiere 已使用 electron-builder，
已依赖 `electron-updater@6.8.9`，Windows 也已经走这套运行时模型。Sparkle 2 是
原生 macOS 应用的主流强方案，但对当前 Electron 架构会额外引入 native bridge、
framework/helper 签名、appcast 和 EdDSA 密钥体系，本项目暂时不值得承担这套复杂度。

没有一手资料支持“Electron/macOS 应用普遍由业务代码直接调用 GitHub REST latest release
API”这一说法。Electron 官方的完整更新路线使用 Squirrel feed 或
`update.electronjs.org`，electron-updater 使用 provider 抽象，Sparkle 使用 appcast；其中
electron-updater 的公开 GitHub provider 源码还明确选择 Releases Atom feed 和
`/releases/latest`，避免使用 GitHub API 限额。直接 REST 调用更准确地说是一个容易实现的
**手动检查版本模式**，不是完整 updater 的行业标准架构。

## 当前项目的事实边界

- 桌面包版本是 `0.2.0`，使用 Electron 43.4.1、electron-builder 26.15.7 和
  `electron-updater` 6.8.9：[`apps/desktop/package.json`](../../apps/desktop/package.json)。
- 现有 updater 只在 `win32` 且正式打包的应用中启用：
  [`windows-updater.ts`](../../apps/desktop/src/main/windows-updater.ts)。
- 当前工作树已经实现 macOS 手动检查：先请求 GitHub REST latest release，失败后跟随
  `github.com/.../releases/latest` redirect，并只接受固定仓库的 release tag 路径：
  [`manual-update-check.ts`](../../apps/desktop/src/main/manual-update-check.ts)。这仍是“手动检查 +
  浏览器下载”，不是自动 updater。
- macOS 当前以 `identity: "-"` 进行 ad-hoc 签名，`notarize: false`，DMG 还明确设置
  `writeUpdateInfo: false`：[`electron-builder.json`](../../apps/desktop/electron-builder.json)。
- macOS 发布只上传 arm64/x64 DMG，没有 ZIP、blockmap 或 `latest-mac.yml`：
  [`.github/workflows/release.yml`](../../.github/workflows/release.yml)。
- 项目已经记录，ad-hoc 签名的应用替换后可能需要重新授予 Screen Recording
  权限：[`CURRENT.md`](../state/CURRENT.md)。
- Settings 当前把版本号硬编码为 `0.2.0-preview.1`，与实际包版本不同。新的
  检查更新 UI 必须使用 main process 的 `app.getVersion()`，不能继续依赖该字面量：
  [`SettingsView.tsx`](../../apps/desktop/src/renderer/src/SettingsView.tsx)。

因此，**当前缺的不只是一次 `checkForUpdates()` 调用**。如果不先补稳定签名与发布
产物，即使应用发现了新版本，也不具备可信的自动替换链路。

## 业界主流方案对比

| 方案 | 更新能力 | 发布数据 | 签名/安全 | 对 Lumiere 的适配度 |
|---|---|---|---|---|
| Electron 内置 `autoUpdater` | macOS 下由 Squirrel.Mac 下载 ZIP 并替换 app | Squirrel JSON feed，或 `update.electronjs.org` 转换公开 GitHub Release | macOS 自动更新要求 app 已签名；Squirrel 验证新 bundle 签名 | 官方、依赖少，但会另建一套与现有 Windows updater 不同的 feed 链路 |
| `electron-updater` / `MacUpdater` | 版本判定、下载进度、缓存、差分下载、channel/staged rollout；底层仍交给 Squirrel.Mac 安装 | `latest-mac.yml` + 每架构 ZIP/blockmap；原生支持 GitHub Releases provider | metadata 中的 SHA-512 保护下载完整性，Squirrel 再验证 macOS code signature | **最适合**：现有依赖、打包器、GitHub 发布和 Windows 实现都可复用 |
| Sparkle 2 | 原生 macOS UI、DMG/ZIP/pkg、delta、channels、phased rollout、最低系统/硬件条件 | Sparkle appcast（RSS/XML）+ archive + EdDSA 签名 | 推荐 Developer ID + notarization + Ed25519；可选签名 feed | 功能强，但 Electron 需 native bridge、helper 签名、appcast 和独立密钥生命周期，当前过重 |
| GitHub API 版本检查 + 浏览器下载 | 只发现新版本，用户手动下载并覆盖安装 | `GET /repos/{owner}/{repo}/releases/latest`，或固定 `releases/latest` 页面 | 检查本身不安装可执行内容；最终 DMG 仍应签名、公证 | **最好的过渡方案**，但不得宣称为“自动更新” |

### 1. Electron 内置 `autoUpdater`

Electron 的 macOS updater 建立在 Squirrel.Mac 上，官方明确要求应用已签名。
`checkForUpdates()` 不是“只查询”：一旦 feed 返回新版本便会自动下载；下载后可用
`quitAndInstall()` 立即安装，也可以留待后续启动时应用。新版 Electron/Squirrel 还支持
feed 提供 `sha256`/`size`、断点续传与可选 delta。

- [Electron `autoUpdater`](https://www.electronjs.org/docs/latest/api/auto-updater/)
- [Electron: Updating Applications](https://www.electronjs.org/docs/latest/tutorial/updates)
- [Squirrel.Mac](https://github.com/Squirrel/Squirrel.Mac)

Electron 还提供了一条简单的官方 GitHub 路线：公开项目把已签名构建发布到 GitHub
Releases，然后通过 `update.electronjs.org` 生成 Squirrel 兼容 feed，或用
`update-electron-app` 减少样板代码。它适合新的简单 Electron 项目，但 Lumiere 已经为
Windows 建立了 `electron-updater` 发布模型，再引入另一个 feed 服务会增加运营分叉。

- [Electron: Publishing and Updating](https://www.electronjs.org/docs/latest/tutorial/tutorial-6-publishing-updating)
- [`update.electronjs.org`](https://github.com/electron/update.electronjs.org)
- [`update-electron-app`](https://github.com/electron/update-electron-app)

### 2. `electron-updater` / `MacUpdater`

electron-builder 的官方文档把 `electron-updater` 与自己生成的 update metadata 作为配套
链路。它原生支持 GitHub Releases provider，同时提供比 Electron 内置 API 更完整的下载
进度、staged rollout 和多平台事件。

macOS 上有一个容易混淆的产物关系：

- DMG 仍是用户首次安装的主要载体；
- updater 运行时实际要求 ZIP；
- `latest-mac.yml` 必须同时列出对应的文件、大小和 SHA-512；
- 仅生成或上传 DMG 不足以启用 macOS 自动更新。

electron-builder 文档明确说明 macOS 需要 ZIP，当前 `MacUpdater` 源码也会在找不到 ZIP 时报
`ERR_UPDATER_ZIP_FILE_NOT_FOUND`。

- [electron-builder: Auto Update](https://www.electron.build/docs/features/auto-update/)
- [`MacUpdater` source](https://github.com/electron-userland/electron-builder/blob/master/packages/electron-updater/src/MacUpdater.ts)

#### arm64/x64 选择

当前 `MacUpdater` 会结合 Rosetta、`process.arch` 和 `uname` 判断 Apple Silicon。如果 metadata
中存在 URL/文件名包含 `arm64` 的文件，Apple Silicon 优先只选它；Intel 则排除
`arm64` 文件。没有 arm64 产物时，Apple Silicon 可回退到 Universal 或可经 Rosetta 执行的
x64 产物。

对 Lumiere 而言，客户端识别不是最大风险；真正需要设计的是：

- updater ZIP 必须继续使用稳定且含 `arm64`/`x64` 的文件名；
- 两个架构的构建不能各自上传并相互覆盖 `latest-mac.yml`；
- 应由一个聚合步骤将两个架构的终态 ZIP 生成为唯一 metadata，再与产物一起上传；
- metadata 与 ZIP 必须来自同一批构建，否则 SHA-512 会不匹配。

本项目当前锁定 6.8.9，实现时应以本地已安装的 API/类型为准，不能直接照搬
electron-updater 7 / electron-builder 27 的新选项。

### 3. Sparkle 2

Sparkle 是原生 macOS 应用中成熟的自动更新框架。它支持 DMG、ZIP、tar、Apple Archive
和 installer package，并提供原生更新 UI、release notes、delta、channels 和 phased
rollout。它使用 appcast（RSS/XML）描述版本，官方推荐用 `generate_appcast` 生成内容、
delta 和 Ed25519 签名。

Sparkle 的安全边界比单纯校验文件哈希更完整：它推荐 HTTPS、Developer ID 签名与公证，
并用内置公钥验证 archive 的 EdDSA 签名；还可选启用签名 appcast/release notes。

- [Sparkle documentation](https://sparkle-project.org/documentation/)
- [Sparkle: Publishing an Update](https://sparkle-project.org/documentation/publishing/)
- [Sparkle: Security and Reliability](https://sparkle-project.org/documentation/security-and-reliability/)

对 Lumiere 的问题不是 Sparkle 能力不足，而是集成成本过高。Sparkle 是 Cocoa/Swift/Objective-C
framework，不是 Electron JavaScript API。当前架构需要新增 native bridge 或赋予 Swift sidecar
应用替换权限，正确嵌入并签名 framework/XPC/helper，同时再运营 appcast 和 EdDSA 密钥。
除非后续出现明确的 macOS-only 原生 UI、复杂分批或 DMG 直接更新需求，否则不应引入。

### 4. GitHub Releases 版本检查及其适用边界

最直接的过渡方案是在 main process 中请求 GitHub REST API：

```text
GET https://api.github.com/repos/Mournerliao/lumiere/releases/latest
```

将返回的 `tag_name` 与 `app.getVersion()` 做 SemVer 比较。发现更新时，只向用户显示新
版本，并打开代码内置的固定地址查看 release notes 和手动下载：

```text
https://github.com/Mournerliao/lumiere/releases/latest
```

GitHub 官方提供 `releases/latest` 稳定链接和“Get the latest release” API。这个接口返回最新的
非 draft、非 prerelease 完整 release。它足以实现一个小型手动版本检查器，但不能据此推导
“Electron 应用普遍直接调用 REST API”：目前没有 GitHub、Electron、electron-builder 或
Sparkle 的一手行业统计支持这种普遍性判断，且各完整 updater 的官方架构并非如此：

- Electron 内置 updater 消费 Squirrel feed；公开 GitHub 项目可通过
  `update.electronjs.org` 把 Releases 转换成该 feed；
- electron-updater 通过 GitHub 或 generic provider 读取 channel metadata；
- Sparkle 读取开发者托管的 appcast；
- electron-updater 当前公开 GitHub provider 的源码明确注明，为避免 API limit，不对
  github.com 使用 REST API，而是读取 Releases Atom feed、`/releases/latest` 和对应 release
  中的 channel file。

- [electron-updater `GitHubProvider` source](https://github.com/electron-userland/electron-builder/blob/master/packages/electron-updater/src/providers/GitHubProvider.ts)

公开数据允许未鉴权请求，但未鉴权 REST API 按**来源 IP**而不是按应用或用户计算，默认限制
是每小时 60 次。在公司、校园、代理等共享 NAT 出口下，这个额度可能被多个用户共享。GitHub
还设有 secondary limits，大多数 REST `GET` 通常计 1 point，部分阈值可不经通知调整。因此：

- 只在手动点击和低频自动时机检查，不密集轮询；
- 可以支持 `ETag` / 条件请求以减少响应体，但 GitHub 只明确保证“正确携带
  `Authorization` 且返回 304”的条件请求不计 primary limit；未鉴权 304 没有这一官方免额
  保证；
- 尊重 `x-ratelimit-*`、`retry-after`，在 403/429 后按 reset 时间或指数退避；
- 网络失败保持静默或只在用户手动检查时显示非阻塞错误；
- 不在客户端嵌入 GitHub token 或 OAuth client secret。

- [GitHub REST API: Releases](https://docs.github.com/en/rest/releases/releases#get-the-latest-release)
- [GitHub: Linking to Releases](https://docs.github.com/en/repositories/releasing-projects-on-github/linking-to-releases)
- [GitHub REST API rate limits](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api)
- [GitHub REST API best practices](https://docs.github.com/en/rest/using-the-rest-api/best-practices-for-using-the-rest-api)

Electron 安全文档明确提醒不要把不可信内容直接传给 `shell.openExternal`。因此不应
从远程 release body 或任意 API 字段中提取 URL 并打开；只打开代码内的 GitHub HTTPS
常量，或严格校验 origin/path 允许列表。

- [Electron Security](https://www.electronjs.org/docs/latest/tutorial/security#15-do-not-use-shellopenexternal-with-untrusted-content)

#### 中国大陆可用性不能由 GitHub SLA 推导

GitHub Status 提供总体组件状态，但当前明确列出的 Enterprise Cloud 区域状态页只有
Australia、EU、Japan 和 US，没有中国大陆。它展示的全球 API uptime 或其他区域数据，不能
解释为中国大陆网络路径的观测数据。

GitHub Online Services SLA 的 99.9% 承诺适用于付费协议覆盖的 GitHub Enterprise Cloud 等
服务，不是对 GitHub Free、公共 Releases 或所有最终用户所在地的普遍承诺。该 SLA 还明确
排除政府行为、GitHub 数据中心外部或客户与数据中心之间的网络/设备故障、第三方服务或带宽
问题、rate limits，以及没有达到实际不可用程度的延迟或性能下降。其 GHEC Service Feature
列表也没有单独列出公共 Releases 资产下载/CDN。

因此，截至本次调研，**没有找到 GitHub 针对中国大陆终端访问 API、Releases 页面或 release
assets 的地域可用性承诺**。这不等于官方声明它一定不可用；准确结论是：GitHub 没有为该路径
提供可依赖的区域 SLA，Lumiere 必须把大陆可达性当作产品侧实测、容错和托管选择问题。

- [GitHub Status](https://www.githubstatus.com/)
- [GitHub Customer Terms](https://github.com/customer-terms)
- [GitHub Online Services SLA](https://github.com/customer-terms/github-online-services-sla)

REST API 的每小时 60 次限制只属于 API primary rate limit，不能直接套用到
`github.com/.../releases/latest` 页面或 release asset 下载；这些仍依赖 GitHub 网络与服务，
但不是同一个限流口径。

#### 对当前 Lumiere 实现的判断

当前 `manual-update-check.ts` 可以**保留为近期 MVP 实现**：它不在客户端嵌入 token，REST
失败时有稳定失败结果，redirect 解析又限制到 `github.com` 和 Lumiere release tag 路径，
满足当前“手动检查、失败不破坏应用”的范围。已有版本比较、IPC 和 Settings 交互也无需因为
托管策略变化而重写。

但它的两条发现路径分别依赖 `api.github.com` 和 `github.com`，所以 redirect fallback 只避开
REST primary rate-limit 口径，**不能作为中国大陆网络可达性的独立 fallback**。也不应将该请求
称为“unmetered”：官方资料只足以说明它不是上述 REST API 60 次/小时的同一限流链路，没有
承诺它完全不受其他流量控制或服务限制。

当“中国大陆正式支持”成为发布条件时，最小演进不是更换 UI 或 updater library，而是按顺序
替换网络边界：

1. 保留现有结果类型、SemVer 比较、IPC 和 Settings UI，只把主版本源替换为自有域名静态
   manifest；GitHub REST/redirect 可降为 best-effort 备用，或完全退出自动检查路径。
2. 保留 GitHub Releases 手动下载，同时增加自有域名的 DMG 镜像；否则只能保证发现新版本，
   不能保证用户完成下载。
3. 获得 Developer ID 与公证能力后，沿用 electron-updater，将 provider 从 GitHub 切为
   `generic`，在同一自有域名/CDN 原子发布 `latest-mac.yml`、ZIP 和 blockmap；GitHub Release
   继续作为公开归档和备用下载。

这个演进路径保留当前实现的大部分产品与代码结构，只逐步把 GitHub 从“唯一运行时依赖”降为
“公开发布与备用分发渠道”。

#### 自有域名 manifest 与托管替代架构

对中国大陆用户，更稳妥的架构是把“发现版本”和“下载安装包”拆开：

| 架构 | 运行时依赖 | 解决的问题 | 仍然存在的风险 |
|---|---|---|---|
| 应用直接请求 GitHub REST | `api.github.com`，下载再依赖 `github.com`/asset host | 无自建服务，开发量最小 | 共享 IP 60/h、无大陆 SLA、API 与下载均受 GitHub 可达性影响 |
| 自有域名静态 manifest + GitHub 手动下载 | 检查依赖自有域名；下载依赖 GitHub | 避开 REST 限流并让“有新版本”提示可控 | **只改善检查**；用户仍可能打不开 Releases 或下载失败 |
| 自有 manifest + 自有对象存储/CDN 下载，GitHub 为备用 | 检查和主下载均走自有域名 | 可独立选择大陆可用的 DNS/CDN/存储并观测端到端表现 | 增加托管、带宽、发布一致性和供应链运维 |
| `electron-updater` generic provider 全自托管 | 自有域名上的 `latest-mac.yml`、ZIP、blockmap | 完整自动更新不依赖 GitHub runtime；复用现有 updater | 必须原子发布匹配的 metadata/artifacts，并维护 HTTPS/CDN |

“自有域名静态 manifest + GitHub 手动下载”技术上完全可行，也是比客户端直调 REST 更好的
近期分层：例如 manifest 只包含 `version`、固定格式的 release notes URL 和两个架构的下载页
信息，由 CI 在 release 发布完成后更新。应用应只信任代码内 allowlist 的 HTTPS origin；如果
manifest 不可达，则把此次检查视为未知而不是“已是最新版”。

但如果目标是**保证大陆用户能完成更新**，只自托管 manifest 不够，至少还需要一个自有下载
入口。GitHub 可以继续作为公开 release 记录、校验和与海外/备用下载源，不必成为运行时唯一
依赖。

electron-updater 已官方支持 generic HTTP(S) provider，而且只需简单文件托管，不要求专用
应用服务器。其 `GenericProvider` 直接从配置的 base URL 获取 channel file，再以同一 base URL
解析 artifacts；这正好适合将 `latest-mac.yml`、ZIP 和 blockmap 放到自有对象存储/CDN。

- [electron-builder: Auto Update](https://www.electron.build/docs/features/auto-update/)
- [electron-updater `GenericProvider` source](https://github.com/electron-userland/electron-builder/blob/master/packages/electron-updater/src/providers/GenericProvider.ts)

Sparkle 同样天然支持自托管：`SUFeedURL` 指向自有 HTTPS appcast，archive URL 可指向 GitHub
或自有存储。若 archive 仍指向 GitHub，它和“自有 manifest + GitHub 下载”一样只解耦版本发现；
将 appcast 和 archive 都放到自有域名才真正移除 GitHub runtime 依赖。

- [Sparkle documentation](https://sparkle-project.org/documentation/)
- [Sparkle customization](https://sparkle-project.org/documentation/customization/)

## 签名与公证是自动替换的门槛

Electron 和 electron-builder 都明确规定，macOS 自动更新需要 code-signed app。Electron 还特别
说明，unsigned 或 ad-hoc signed app 在依赖系统身份的 API 上可能表现不一致，Squirrel.Mac
要求签名才能工作。实际生产链路应使用连续且一致的 Developer ID Application identity，
而不是将当前 `identity: "-"` 视为可用的生产更新身份。

- [Electron: Code Signing](https://www.electronjs.org/docs/latest/tutorial/code-signing)
- [Electron `autoUpdater`](https://www.electronjs.org/docs/latest/api/auto-updater/)
- [electron-builder: Auto Update](https://www.electron.build/docs/features/auto-update/)

notarization 不是 `checkForUpdates()` 这个 API 的技术前置，但它是现代 macOS 站外生产分发
不应略过的发布门槛。Apple 要求提交公证的软件使用 Developer ID（不是 ad-hoc、
Apple Development 或本地证书）、Hardened Runtime、安全时间戳和有效的嵌套签名。

- [Apple: Signing Mac Software with Developer ID](https://developer.apple.com/developer-id/)
- [Apple: Notarizing macOS Software Before Distribution](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)

SHA-512 或 SHA-256 只证明下载字节与 metadata 相符，不能单独证明更新的发布者身份。
对 `electron-updater` 链路，真正的执行身份保障来自 Squirrel/macOS code signature 验证；
Sparkle 则另外用 EdDSA 把 archive 与 app 内置公钥绑定。

## 推荐实施路线

### 阶段 A：现在可以交付的检查更新

1. 保留当前 main-process 手动检查服务和 SemVer 比较。当前版本先以 GitHub latest release
   API + redirect fallback 作为 best-effort 版本源；正式承诺大陆支持前，将主版本源换成
   自有域名静态 manifest。
2. 通过有类型 IPC 向 renderer 暴露 `currentVersion`、检查状态、`availableVersion` 和错误；
   只允许 main process 发起网络请求和打开外部 URL。
3. Settings 的 System/About 区域使用 `app.getVersion()`，增加“Check for updates”。
4. 发现新版本后显示下载操作。近期可继续打开固定 `releases/latest` 页面；大陆正式支持至少
   增加一个自有 DMG 下载入口，GitHub 作为备用。
5. 自动检查如果启用，应低频、静默；“已是最新版”和网络失败只在用户手动点击时反馈。

这一阶段不需改发布产物，也不会把未验证的应用替换写入用户机器。

### 阶段 B：Developer ID 与公证基础

1. 申请并固定 Developer ID Application identity，以 GitHub Actions secrets 管理证书与公证凭证。
2. 替换 ad-hoc 签名，对 Electron app、Swift Host 和所有嵌套 Mach-O 完成签名。
3. 公证并 staple 最终发布载体，保留当前 Hardened Runtime。
4. 用真实下载的过渡构建验证 Gatekeeper、Screen Recording 身份连续性和覆盖安装。

当前公开的 `v0.2.0` 没有 macOS updater 代码，且不应依赖其 ad-hoc 签名完成安全自动
迁移。因此第一个 Developer ID 签名且携带 updater 的版本，仍需要现有用户手动下载安装；
从该版本到下一版本，才是自动更新的第一个有效端到端验证。

### 阶段 C：`electron-updater` macOS 链路

1. 将 macOS target 扩展为 DMG + ZIP。若 GitHub 可达性只需 best-effort，可配置公开 GitHub
   provider；若大陆可用性是发布条件，直接采用自有域名的 generic provider。
2. arm64/x64 构建后由单一聚合步骤产出最终 `latest-mac.yml`，与 ZIP/blockmap 一起上传；
   DMG 继续保持现有下载名称和首次安装作用。
3. 将当前 Windows-only updater 收敛为共享的 main-process update service，但保留各平台安装
   语义。对 macOS 使用 `electron-updater` 6.8.9 已有事件和配置，不提前升级依赖。
4. 用户交互采用“检查 → 发现 → 下载进度 → 重启安装/稍后”；自动检查无更新时保持静默。
5. 记录 updater 日志，但不将 token、请求凭证或不受控的 release body 记录到本地。

自托管 generic provider 不改变签名、公证、SHA-512 或架构验证要求；它只替换 metadata 与
payload 的网络来源。发布必须保证旧 manifest 在新一批 ZIP 全部可下载前不指向新版本，避免
客户端看到尚未完整上传的 release。

## 必须的验证边界

自动更新不能通过“最新版本能启动”来证明。最低验证矩阵是：

| 路径 | 必须观察的结果 |
|---|---|
| arm64 `N → N+1` | 发现正确版本、选中 arm64 ZIP、完整性校验、重启替换、Host 架构正确 |
| Intel x64 `N → N+1` | 不选中 arm64 产物，替换后原生启动和 Host 连接正常 |
| Apple Silicon + Rosetta（如继续支持） | 更新后的架构迁移行为与政策一致，不误下载不可运行产物 |
| 网络中断/校验失败 | 旧版本保持可用，不留下半替换 bundle，下次检查可恢复 |
| 权限连续性 | 更新前后分别实测 Display/Region 截图和 Screen Recording 权限，不从签名理论推导结论 |
| 发布一致性 | GitHub Release 中 DMG、ZIP、blockmap、`latest-mac.yml` 与 checksum manifest 全部对应同一版本/构建 |

每个架构都要使用“已安装的旧版本”通过真实网络更新到“已发布的新版本”。仓库单测、
打包成功、新版本 fresh install 和单一架构的观察，都不能代替这一验证。
