# 快捷键到 Region 遮罩：延迟诊断

- 日期：2026-09-14；源码基线：`e6d379d`，桌面包版本 `0.5.0`，Electron `43.4.1`。
- 范围：当前运行机制、macOS 原生准备实测、Windows 源码与既有证据、社区实现比较；不修改运行代码。
- 结论：当前实现把较多原生准备与预览编码放在可交互之前。已有直接证据表明 Electron 之外就有明显耗时；尚不能量化用户实际快捷键延迟中 Electron 的占比。

## 当前路径

```text
全局快捷键回调 → CaptureSession.run → captureRegion
  → 隐藏主窗口
  → 并行启动 Overlay.ensureReady
  → resolveRegionTarget → Host.getCapabilities（串行前置）
  → Host.prepareRegion
      → 校验目标 → 获取并保留完整 native frozen frame
      → 派生逻辑尺寸 sRGB 预览 → PNG 编码 → 临时文件写入
  → 等待 native preparation 和 Overlay ready 两者完成
  → 设置 Overlay bounds → IPC 激活 renderer
  → img 请求受控 URL → main readFile → Response → Chromium 图片加载/解码
  → 两次 requestAnimationFrame → renderer ready IPC
  → BrowserWindow.show / focus → OS compositor 实际显示
用户框选完成 → commitRegion 从同一 native frozen frame 裁剪和输出
```

入口与编排见 [main/index.ts](../../apps/desktop/src/main/index.ts#L652)、[router](../../apps/desktop/src/main/capture-command-router.ts#L68)、[renderer](../../apps/desktop/src/renderer/src/RegionOverlay.tsx#L128)。

已经存在的优化不能忽略：

- [NativeProcessPlatformHost](../../apps/desktop/src/main/native-process-platform-host.ts#L149) 复用常驻子进程，不是每按一次快捷键重新启动 Swift/.NET。
- [Overlay controller](../../apps/desktop/src/main/region-overlay-controller.ts#L25) 预热并复用隐藏 BrowserWindow，`backgroundThrottling: false`。准备时与 native 路径并行。
- 预热绑定主窗口 `ready-to-show`；[MainWindowPresentation](../../apps/desktop/src/main/main-window-presentation.ts#L25) 不要求用户实际打开主窗口才触发。不能把旧 ADR 的“首屏展示后”字面描述当作当前行为。
- macOS 复用 CIContext；Windows 复用 capture engine / D3D device。Windows engine 在首次实际 capture 时惰性创建，因此首次抓取仍有初始化成本。
- 预览已缩至逻辑尺寸，正式截图仍从完整原帧裁剪。

## 本轮实测

### 方法与边界

环境为本机 macOS `27.0 (26A428)`，使用 `/Applications/Xcode-beta.app` 构建当前源码 Release Host。目标由 Host 报告为 SDR，完整帧 `5120×2880`，预览 `2560×1440`，PNG 约 4.15 MB。本轮没有测内置 XDR 的 HDR 路径，也没有 Windows 运行环境。

通过 JSON Lines 驱动真实 `getCapabilities → prepareRegion → cancelRegion`。每组 6 次，第一份单列，后 5 份报告中位数及范围。一次只保留一份冻结帧，取消后预览被清理，没有交付或上传截图。两组仅改变是否在计时前额外调用一次 `getCapabilities` 填充预热缓存。

这测的是**原生可用预览的准备时间**，不含 Electron 图片解码、窗口显示或真实键盘事件，不是完整端到端复现。探针的 200 ms 预算仅用于暴露原生准备已占据多少响应预算，并非既有产品验收标准。样本不足以报告 p95，也不能把不同场景/构建的细小差异当作因果收益。

实际执行：

```sh
DEVELOPER_DIR=/Applications/Xcode-beta.app/Contents/Developer swift build --package-path hosts/macos -c release
python3 /private/tmp/lumiere-region-latency.py hosts/macos/.build/release/LumiereMacHost
python3 /private/tmp/lumiere-region-latency.py hosts/macos/.build/release/LumiereMacHost --prewarm
```

构建成功（7.73 秒）。首个沙箱内探针无法解析显示目标；经授权在沙箱外运行后，真实准备与取消成功。探针和原始阶段日志保留在本次机器的 `/private/tmp/lumiere-region-latency.py`、`lumiere-region-latency-current.jsonl`、`lumiere-region-latency-prewarmed.jsonl`，这些是临时诊断产物，未纳入源码。

Release Host SHA-256：`79e210467a43d8a1856115e09a6ca850ede2bb8c24e3242196013874bb03f4f1`。

### 数值

| 阶段 | 无提前填充：热中位数 | 提前填充：热中位数 |
| --- | ---: | ---: |
| getCapabilities | 36.7 ms | 0.2 ms |
| prepareRegion 整体 | 177.4 ms | 167.2 ms |
| 其中：获取完整帧 | 61 ms | 56 ms |
| 其中：预览 render | 20 ms | 18 ms |
| 其中：PNG encode | 94 ms | 90 ms |
| 其中：临时文件 write | 2 ms | 2 ms |
| capabilities + prepare | **214.5 ms** | **167.4 ms** |
| 合计热样本范围 | 205.9–215.7 ms | 165.9–172.2 ms |

各阶段中位数独立计算，不保证恰好相加等于总计中位数。

无提前填充时，首次请求共 853.1 ms，其中 capabilities 591.9 ms、prepare 261.2 ms；这包含新进程启动和初始化，是单个冷样本，**不是已常驻应用每次快捷键的正常延迟**。提前填充组把进程/能力初始化移到计时之前，首次 prepare 仍需 242.8 ms；该数字不能与前者作为同起点的冷启动对比。

热样本原始总计：无提前填充 `[214.5, 215.7, 214.6, 205.9, 206.8]` ms；提前填充 `[165.9, 167.4, 169.1, 166.6, 172.2]` ms。

### 安装版本与历史证据

本机 `/Applications/Lumiere.app` 的 Info.plist 实际是 **0.3.2**，`artifacts/macos/apps/arm64/Lumiere.app` 是 **0.4.0**。这些与当前源码的 0.5.0 不同，不能假定用户正运行其中某一份，也没有执行升级或替换安装。旧安装 Host 的独立探针热中位数为 214.8 ms（未提前填充），只作为旁证。

[CURRENT.md](../state/CURRENT.md) 已记录此前 macOS Region 热中位数 343.5 ms（311–415 ms），Windows 优化后的 **Region preparation** 热中位数 666 ms。Windows 这个数不是本轮端到端 Overlay 测量，更不能与本轮 macOS 原生 167 ms 当作同口径平台排名。

## 问题与证据强度

### 1. PNG 是当前 macOS 热路径最大的已测单项

[prepareRegion](../../hosts/macos/Sources/LumiereMacHostCore/MacCaptureService.swift#L166) 必须等 [encodePNG](../../hosts/macos/Sources/LumiereMacHostCore/MacCaptureService.swift#L564) 和 write 完成才返回。即使只有逻辑尺寸，2560×1440 仍是 369 万像素；本轮 PNG 编码占预热命中原生总准备约 54%。其后 Chromium 还要解码。

这属于**预览传输实现的成本**。高端 GPU 不会自动加速 ImageIO PNG 压缩。本轮没有 profiler 证明编码器内部线程/GPU分配，因此只确认阶段耗时，不杜撰其内部实现。

只改临时文件为内存 PNG，只能移除写/读与部分复制，保留最大的编码成本。本轮写入约 2 ms；不能以“去掉磁盘”许诺百毫秒改善。真正应比较的是无损低压缩预览、raw SDR pixels、或 GPU preview surface 的完整 encode/transport/decode 成本。

### 2. Windows 的预览计算路径没有利用 5080 执行色彩转换

[preview encoder](../../hosts/windows/src/Lumiere.Windows.Graphics/Output/IRegionPreviewEncoder.cs#L40) 先 `ReadRgba16Float(texture, cropRegion: null)`，然后调用 CPU converter。具体为：

1. [readback](../../hosts/windows/src/Lumiere.Windows.Graphics/Output/CapturedFrameTextureReadback.cs#L86) 每次创建全尺寸 staging texture，CopySubresourceRegion 后立即以无 DO_NOT_WAIT 标志的 Map 读回，再逐行 Marshal.Copy 到托管 byte array。
2. 4K RGBA16F 全图约 **63.3 MiB**；预览虽然是 2560×1440，这份全尺寸读回没有缩小。
3. [pixel converter](../../hosts/windows/src/Lumiere.Windows.Graphics/Output/SrgbVisualMatchPixelConverter.cs#L36) 为普通嵌套 CPU for 循环，每个输出像素对四通道做双线性采样，再做 normalization、tone mapping、sRGB 转换；非线性分支使用 MathF.Pow。源码没有 GPU shader 或显式并行/SIMD 实现；不排除 JIT 的局部优化。
4. 随后还要 PNG 编码、写文件、Chromium 解码及上传。

因此“5080 很快”不能抵消 CPU 色彩循环和 GPU→CPU 同步读回。Map 的同步行为见 [Microsoft 文档](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-map)。**源码确认这条成本链存在；各项在用户 Windows 上的具体耗时和最大瓶颈仍未测量。**

首选验证方向是由 native GPU 先缩放/完成 sRGB Visual Match，再只读回逻辑尺寸 BGRA8；即使暂时保留 PNG，也能减少读回与 CPU 转换工作。须验证现有色彩语义，不能只做速度对比。

### 3. 抓帧资源没有全程保持热状态

macOS 每次 [acquireFrozenFrame](../../hosts/macos/Sources/LumiereMacHostCore/MacCaptureService.swift#L395) 仍调用 SCScreenshotManager.captureImage；CIContext/枚举预热不等于已经有一张可用新帧。本轮热抓帧约 56 ms，首次明显更高。

Windows 每次 [StartCapture](../../hosts/windows/src/Lumiere.Windows.Capture/CaptureService.cs#L225) 都建 WinRT device wrapper、FramePool 和 CaptureSession，等待第一帧，再经 session disposal 返回 held texture。D3D device 本身复用，不能笼统说所有资源每次重建。Microsoft 工程师维护的 [Win32CaptureSample snapshot](https://github.com/robmikh/Win32CaptureSample/blob/main/Win32CaptureSample/CaptureSnapshot.cpp) 也采用单次 session 获取首帧，这不是错误用法，但可能不适合极低延迟目标。

常驻 SCStream/WGC 可以作为抓帧阶段的对照实验，而不是已证实的修复。它会改变后台资源占用、系统捕获指示与帧新鲜度约束；若拿到快捷键前的旧帧，也会破坏冻结时刻语义。当前测量不足以要求立即采用。

### 4. macOS 预热缓存是一次性消费，能力查询不一定便宜

[capabilities](../../hosts/macos/Sources/LumiereMacHostCore/MacCaptureService.swift#L105) 会 await warmScreenCaptureKitIfAuthorized；acquireFrozenFrame 消费 warmedShareableContent 后置 nil。所以下次若没有后台填充，快捷键里的能力查询会承担枚举成本。本轮空缓存热查询约 37 ms，命中约 0.2 ms。

但主进程在完成/取消后会刷新 tray 和 capture surface，刷新本身会调用 getCapabilities，有机会提前补充缓存。因此**不能说现有每次快捷键都额外损失 37 ms**；应记录真实快捷键的 cache hit/miss 与能力查询段，再决定是否加强 native 缓存失效和预热机制。也不能把 getCapabilities 直接删掉，它还签发目标 token 与验证目标状态。

### 5. Electron 显示阶段仍有附加等待，但未证明是主因

当前 [renderer](../../apps/desktop/src/renderer/src/RegionOverlay.tsx#L137) 等 onLoad → decode → 两次 rAF 才报告 ready。正常 60 Hz 调度下这两个回调大约涉及 1–2 个帧周期，实际取决于调度相位与负载；不能将其固定算作 33 ms。它们可能用于保证背景绘制就绪，删除前应验证首帧白/黑闪。

窗口预热到 1×1、激活时改为全屏尺寸，因此 document ready 不代表全屏纹理、图片、OS compositor 已预热。backgroundThrottling 已关闭，不能无证据归咎于默认后台节流。

没有发现快捷键路径人为 sleep 300/500 ms、重复启动 Electron 或热路径反复 loadURL。UI shell 的额外成本需要当前版本端到端 trace，而不是以框架名推定。

### 6. 现有计时不足以回答“按键到眼睛看到遮罩”

- main 的 command-received 起于 captureRegion 内部，不是 OS 物理按键时刻。
- main 等待 resolveRegionTarget，但没有独立报告 capabilities/resolved 段；native 计时则从 prepareRegion 内开始，因此单看 native 日志会漏掉前置查询。
- Windows 首次 GetOrCreateEngineAsync 发生在 engine PrepareRegionAsync 计时之前。
- Windows preview-rendered 合并了 readback 和 CPU conversion；frame-acquired 合并目标/HDR准备、session start、首帧以及退出清理，无法直接定位其中一项。
- overlay-ready 合并图片加载、decode、React/IPC/rAF；overlay-shown 在 show/focus 调用返回后立刻记录，不是 compositor 提交或真实屏幕呈现确认。
- 没有正式的冷/热交互预算。旧 ADR 说“满足 latency budget”但没有给出可持续验收的数字；减少耗时与达到用户响应预期不是同一个结论。

建议后续最小测量切分：hotkey callback、capabilities start/end/cache hit、native acquire、readback、convert、encode、write/read、decode、renderer ready、show、首帧呈现。端到端可先用单调时钟软件边界，再以高速录屏/外部相机验证可见时刻。无需建设全仓性能平台。

## 业界对照与 Electron 判断

[Flameshot](https://github.com/flameshot-org/flameshot/blob/master/src/widgets/capture/capturewidget.cpp) 用内存 QPixmap 绘制冻结背景；[ksnip](https://github.com/ksnip/ksnip/blob/master/src/backend/imageGrabber/AbstractRectAreaImageGrabber.cpp) 的冻结模式传 QPixmap background。它们证明“先冻结再框选”不要求 PNG 文件，不能证明其传统管线满足本项目 HDR 契约或必然更快。

本轮进一步核实同为 Electron 的 **eSearch**：普通预览实际走 native capture → raw pixels → ImageData → Canvas → 显示，避开 PNG 中间文件。它的 renderer 安全配置不同，不能直接搬来替换本项目的 sandbox/native Host 边界。

**更关键的是，当前 Electron 43.4.1 已提供公开但 Experimental 的 sharedTexture 导入 API。** 旧文“没有公开接口”的事实判断失效。可导入本地 IOSurfaceRef / NT HANDLE，经 main 传给 sandbox renderer 产生 VideoFrame，但外部 Host→main 仍需 native handle 传输与生命周期契约；官方该版本测试的 macOS arm64 gate 不能作为 Windows 验证。

固定版本链接、eSearch 源码链、sandbox/handle/颜色限制见本轮独立的 [预览传输复核](region-preview-transport-2026-09.md)。因此目前证据支持**优先改进预览生产与传输链，保留 Electron shell**；没有证据支持为了这次延迟直接整应用重写原生 UI。

## 建议的优化顺序

| 顺序 | 工作 | 理由与验收 |
| --- | --- | --- |
| 1 | 当前版本完整快捷键阶段测量，Windows 单独采样 | 得到真实体验基线，避免漏掉 capabilities、初始化、首帧呈现。分别记录常驻首次、连续热运行、空闲后重入。 |
| 2 | Windows 对照 GPU preview render / reduced readback | 针对已确认的全帧 CPU 路径；比较 readback+convert 耗时及相同冻结帧的色彩/几何。 |
| 3 | macOS 对照无损预览编码成本与无编码预览桥接 | 本轮最大已测热项为 PNG；先比较完整成本，不只比较文件 I/O。不预设 PNG/WebP/raw/sharedTexture 谁一定赢。 |
| 4 | 根据实际 cache miss 与 decode/rAF 数据削减剩余等待 | 缓存不能失去目标/拓扑校验；首帧策略不能引入闪屏。 |
| 5 | 只有 acquire 仍超预算时，再评估常驻 capture stream | 明确额外资源、系统指示与冻结时刻契约；最后才评估 native overlay。 |

建议把“常驻热运行到可交互遮罩 ≤150 ms”作为下一轮可讨论目标，冷路径单列；这是提议，不是已验收承诺。本轮原生预热命中就需要约167 ms，说明仅优化 React 或删除两个 rAF 不足以保证该目标。

本轮产出为诊断与研究文档。运行代码未改，没有声称修复已完成；剩余不确定性集中在当前实际运行应用的身份、完整可见帧延迟、Windows 阶段占比及 HDR 场景。
