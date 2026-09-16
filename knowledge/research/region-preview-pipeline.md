# Region Preview Pipeline：冻结帧不等于全尺寸 PNG 往返

> 2026-09-14 复核：本文保留为原方案的调研依据，其中“Electron 没有公开 shared texture 导入 API”的判断对当前 43.4.1 已不成立；见[固定版本传输复核](region-preview-transport-2026-09.md)。逻辑尺寸 PNG 优化后的实际瓶颈与后续建议见[本轮延迟诊断](region-capture-latency-2026-09.md)。下文的 full-resolution 路径属于优化前实现，不代表当前运行代码。

- 调研日期：2026-09-04
- 跟踪 Issue：[GitHub Issue #15](https://github.com/sousouliao/lumiere/issues/15)
- 范围：Flameshot、ShareX、ksnip 的官方源码，Apple ScreenCaptureKit/Core Image/IOSurface，以及 Windows.Graphics.Capture/D3D11
- 目标：判断 Region Overlay 出现前是否必须把完整帧正式转换、编码为 PNG、写盘再解码，并为 Lumiere 的 native HDR authoritative frame + Electron Overlay 架构选择更合适的预览路径

## 结论

**“先冻结完整桌面，再让用户框选”是成熟截图工具的常见交互；“Overlay 出现前先把完整帧编码成 PNG、写入磁盘，再由 UI 读回并解码”不是实现该交互的必要步骤，也不是本次核对到的开源工具所采用的数据路径。**

## 实施结果

本轮已采用 logical-size lossless preview：macOS 用复用的 `CIContext` 完成 Visual
Match 与 Lanczos 缩放，Windows 从 float readback 直接线性采样到逻辑尺寸并融合完成
normalization、tone map 和 sRGB/BGRA8 转换。Electron Overlay 改为首屏后预热、隐藏复用，
并用 generation-scoped 会话拒绝迟到事件。完整 native frozen frame、commit 的全分辨率裁剪
以及临时 PNG/token 安全桥接均保留。决策与取舍见
[`../decisions/0014-logical-region-preview-and-reusable-overlay.md`](../decisions/0014-logical-region-preview-and-reusable-overlay.md)。

Flameshot、ShareX 和 ksnip 都先获得一张完整屏幕位图，并把这张进程内位图直接作为选择界面的背景；选区确认后，再从同一位图裁剪或渲染结果。它们分别使用 Qt `QPixmap` 或 .NET `Bitmap`，源码中没有“为了显示选区背景而先保存完整 PNG 再读回”的步骤。PNG/文件保存出现在确认后的导出阶段，而不是选择 UI 的前置条件。[Flameshot `CaptureWidget`](https://github.com/flameshot-org/flameshot/blob/master/src/widgets/capture/capturewidget.cpp) · [Flameshot `CaptureContext`](https://github.com/flameshot-org/flameshot/blob/master/src/tools/capturecontext.h) · [ShareX `RegionCaptureForm`](https://github.com/ShareX/ShareX/blob/master/ShareX.ScreenCaptureLib/Forms/RegionCaptureForm.cs) · [ksnip `AbstractRectAreaImageGrabber`](https://github.com/ksnip/ksnip/blob/master/src/backend/imageGrabber/AbstractRectAreaImageGrabber.cpp)

因此，Lumiere 当前路径中的问题不是 capture-before-overlay 或“保留静止帧”这个产品决策，而是预览桥接粒度：当前两端 Host 都在 `prepareRegion` 阶段对 **full backing-pixel frame** 做完整 Visual Match、PNG 编码和临时文件写入，Electron main 又把文件整体读成 Buffer，Chromium 再解码后才显示 Overlay。这个路径保持了安全边界和同帧语义，但把原本只需覆盖屏幕逻辑尺寸的交互预览，当成了完整分辨率交付 artifact 来处理。[macOS 当前实现](../../hosts/macos/Sources/LumiereMacHostCore/MacCaptureService.swift) · [Windows 当前实现](../../hosts/windows/src/Lumiere.Windows.Capture/WindowsDisplayCaptureEngine.cs) · [Electron 当前实现](../../apps/desktop/src/main/index.ts)

对 Lumiere 的优先建议是：

1. 继续由 native Host 持有 full-resolution authoritative HDR/SDR frame，commit 仍从它裁剪并生成正式 artifact。
2. `prepareRegion` 只派生一张 **target logical-size、RGBA8/sRGB 的 SDR preview**，再做无损 PNG 编码并沿用现有受控临时文件/token 通道。
3. 先用分段计时验证缩小后的 conversion、encode、write、read、decode 各自占比；只有磁盘/encoded bridge 仍明显超标时，再升级到内存二进制通道或 native surface。
4. 不建议 MVP 直接把 IOSurface/DXGI shared texture 接入 Electron；平台具备跨进程共享能力，但 Electron 的公开图片 API 面向 PNG/JPEG/bitmap buffer，没有公开的任意 IOSurface/DXGI texture → DOM image 导入契约，落地需要新的 native/Chromium 集成边界。[Electron `nativeImage`](https://www.electronjs.org/docs/latest/api/native-image) · [Apple IOSurface](https://developer.apple.com/documentation/iosurface) · [Microsoft `CreateSharedHandle`](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgiresource1-createsharedhandle)

## 1. 三个开源工具实际怎样做

### Flameshot：抓成 `QPixmap`，Widget 直接绘制，确认后才导出

Flameshot 在创建 `CaptureWidget` 时同步调用 `ScreenGrabber::grabEntireDesktop`，把结果放进 `m_context.screenshot`，同时保存 `origScreenshot`。它随后创建选择、放大镜和工具 UI；`paintEvent` 直接执行 `painter.drawPixmap(..., m_context.screenshot)` 绘制冻结背景。确认时 `selectedScreenshotArea()`/`pixmap()` 从同一个上下文得到选中区域，窗口销毁后才进入 `exportCapture`。[构造与抓帧](https://github.com/flameshot-org/flameshot/blob/master/src/widgets/capture/capturewidget.cpp#L100-L122) · [直接绘制 QPixmap](https://github.com/flameshot-org/flameshot/blob/master/src/widgets/capture/capturewidget.cpp#L617-L643) · [`CaptureContext` 的两张 QPixmap](https://github.com/flameshot-org/flameshot/blob/master/src/tools/capturecontext.h#L10-L17)

其导出代码只有在 `PRINT_RAW` 或 `SAVE` 等任务发生时才调用 PNG 保存；这位于选区完成后的 `exportCapture`，不是 Overlay 展示前的中间格式。[Flameshot `exportCapture`](https://github.com/flameshot-org/flameshot/blob/master/src/core/flameshot.cpp#L426-L458)

可确认的数据路径是：

```text
screen grab -> QPixmap in CaptureContext -> QWidget/QPainter draws pixmap
            -> select/crop -> save/copy/upload (PNG only when requested)
```

### ShareX：`Bitmap Canvas` 就是冻结背景，选区后从 Canvas 生成结果

ShareX 的 `RegionCaptureForm` 拥有 `Bitmap Canvas`。如果调用方没有传入 canvas，构造器直接用 `Screenshot().CaptureRectangle(ScreenBounds)` 获得屏幕 `Bitmap`；`InitBackground` 以 `TextureBrush(Canvas)` 作为背景，暗化效果也只是 clone 一张内存 Bitmap 后绘制半透明层。[Canvas acquisition](https://github.com/ShareX/ShareX/blob/master/ShareX.ScreenCaptureLib/Forms/RegionCaptureForm.cs#L96-L123) · [内存背景初始化](https://github.com/ShareX/ShareX/blob/master/ShareX.ScreenCaptureLib/Forms/RegionCaptureForm.cs#L312-L372)

选区完成后，`GetResultImage` 才调用 `ApplyRegionPathToImage(Canvas, ...)`，或对渲染后的 Bitmap 执行 `CropBitmap`。保存、复制、上传接口接收的也是这个结果 `Bitmap`；文件编码属于后续 task，不是 form 启动的门槛。[ShareX `GetResultImage`](https://github.com/ShareX/ShareX/blob/master/ShareX.ScreenCaptureLib/Forms/RegionCaptureForm.cs#L1391-L1443) · [保存/复制发生在结果生成之后](https://github.com/ShareX/ShareX/blob/master/ShareX.ScreenCaptureLib/Forms/RegionCaptureForm.cs#L1486-L1528)

可确认的数据路径是：

```text
screen capture -> in-memory Bitmap Canvas -> TextureBrush paints selection form
               -> region path/crop -> result Bitmap -> after-capture tasks
```

### ksnip：冻结模式先抓 `QPixmap background`，完成后直接 `copy(rect)`

ksnip 明确区分透明实时背景模式与冻结背景模式。当 `freezeImageWhileSnippingEnabled()` 为真时，`openSnippingArea` 先对全屏区域调用 `getScreenshotFromRect` 得到 `QPixmap background`，然后把它传给 `showWithBackground(background)`；用户完成选区后，`getScreenshotFromBackground` 直接返回 `snippingAreaBackground().copy(mCaptureRect)`。没有编码或文件往返。[ksnip 冻结背景完整路径](https://github.com/ksnip/ksnip/blob/master/src/backend/imageGrabber/AbstractRectAreaImageGrabber.cpp#L101-L213)

ksnip 的 changelog 也把这项能力命名为 “Freeze image while selecting rectangular area”，说明静止背景是显式产品模式；但实现仍是内存 QPixmap，而不是预先产出 PNG artifact。[ksnip changelog](https://github.com/ksnip/ksnip/blob/master/CHANGELOG.md)

可确认的数据路径是：

```text
screen grab -> QPixmap background -> snipping window paints background
            -> background.copy(selection) -> CaptureDto
```

### 证据边界

这些项目足以回答“冻结 UX 是否要求 PNG/磁盘往返”：不要求。但它们大多是单进程、传统 SDR 位图管线，不能直接证明 HDR-aware、native Host 与 Chromium 分进程的最佳实现。对 macOS Screenshot、Windows Snipping Tool 等闭源工具，只能观察到冻结后框选的 UX；在没有官方实现文档或源码时，不能据此外推其内部是否使用 IOSurface、共享纹理或编码预览。

## 2. 平台原生能力并不要求先编码

### macOS

`SCScreenshotManager.captureImage` 直接把单帧交付为 `CGImage`，`captureSampleBuffer` 则交付 `CMSampleBuffer`；API 本身没有要求先生成文件。[Apple `SCScreenshotManager`](https://developer.apple.com/documentation/screencapturekit/scscreenshotmanager)

拿到 `CGImage` 后，原生 UI 可以直接把它设为静态 `CALayer.contents`，无需 PNG encode/decode。Core Image 也能把 `CIImage` 直接 render 到 bitmap、`CVPixelBuffer`、`IOSurface` 或 `MTLTexture`；这意味着 tone-map/downscale 的目标可以是显示 surface，而不必是编码器。[Apple `CALayer.contents`](https://developer.apple.com/documentation/quartzcore/calayer/contents) · [Apple `CIContext`](https://developer.apple.com/documentation/coreimage/cicontext)

IOSurface 是 Apple 明确定义的跨进程 framebuffer/texture 共享机制，技术上可让 native producer 和另一个进程共享硬件加速 buffer；但它只解决 OS resource sharing，不自动提供 Electron DOM 可消费的图片 URL 或安全生命周期。[Apple IOSurface](https://developer.apple.com/documentation/iosurface)

### Windows

Windows.Graphics.Capture 通过 `Direct3D11CaptureFramePool` 交付 `Direct3D11CaptureFrame`。每帧直接包含 `Surface`，Microsoft 的示例把该 surface 转为 Win2D `CanvasBitmap` 后显示，并要求应用在 frame 归还 pool 前复制需要保留的 `ContentSize` 区域。这个模型同样是 surface/texture-first，而不是 encoded-image-first。[Microsoft Screen capture：Acquire/Process frames](https://learn.microsoft.com/en-us/windows/apps/develop/media-authoring-processing/screen-capture#process-capture-frames) · [`Direct3D11CaptureFramePool`](https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.direct3d11captureframepool)

D3D11/DXGI 允许为 shared texture 创建 NT handle，再由另一个 D3D device 打开，因此跨进程 GPU texture 在平台层可行；但需要明确的 handle 权限、同步、设备兼容和释放协议。[Microsoft `IDXGIResource1::CreateSharedHandle`](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgiresource1-createsharedhandle) · [Microsoft `IDXGIResource1`](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nn-dxgi1_2-idxgiresource1)

## 3. Lumiere 当前路径为何代价偏高

Lumiere 当前 Region 的正确性模型没有问题：native Host 持有 authoritative frame，Overlay 只拿 preview，commit 从原帧裁剪。这保证了时间同帧，也避免 renderer 接触 HDR truth。

代价来自 prepare 阶段把 preview 做成了 full-resolution artifact：

```text
native full-resolution HDR/SDR frame
  -> full-frame Visual Match to RGBA8/sRGB
  -> full-resolution PNG encode
  -> temporary file write
  -> Electron main readFile into Buffer
  -> protocol Response
  -> Chromium PNG decode
  -> img upload/composite
  -> Overlay show
```

macOS 的 `prepareRegion` 调用完整 `makeVisualMatchPNG` 并写临时文件；commit 又从冻结原帧的 crop 生成正式 PNG。Windows 同样先对 held texture 执行 `EncodePngAsync(cropRegion: null, VisualMatchContext)`，再写临时文件。[macOS `prepareRegion`](../../hosts/macos/Sources/LumiereMacHostCore/MacCaptureService.swift) · [Windows `PrepareRegionAsync`](../../hosts/windows/src/Lumiere.Windows.Capture/WindowsDisplayCaptureEngine.cs)

在 2x Retina 上，若目标逻辑尺寸为 2560×1440，而 backing frame 是 5120×2880，full-resolution RGBA8 为约 56.25 MiB，logical-size RGBA8 约 14.06 MiB；预览缩到逻辑尺寸可把需要 tone-map/render、压缩、传输、解码与纹理上传的像素数降到四分之一。最终 artifact 不受影响，因为 full-resolution authoritative frame 仍保留在 Host。

此外，Electron 当前 handler 使用 `readFile` 把整个 PNG 读成一个 Buffer 后构造 Response；即使磁盘很快，这仍会产生一份完整 encoded buffer 和 Chromium decode。Electron protocol 可以返回 Response，旧的 protocol response 契约也支持 file path 或 readable stream，因此后续可单独评估 streaming/file response 来减少 main-process 峰值复制，但它不能消除前面的 PNG encode 或后面的 decode。[Electron `protocol.handle`](https://www.electronjs.org/docs/latest/api/protocol) · [Electron `ProtocolResponse`](https://www.electronjs.org/docs/latest/api/structures/protocol-response)

## 4. 可选方案与权衡

| 方案 | Overlay 前工作 | 优点 | 代价 / 风险 | 结论 |
|---|---|---|---|---|
| A. **logical-size lossless preview + 临时文件** | full frame 保留；只对逻辑尺寸预览做 SDR render、PNG encode/write/decode | 最小改动；保留现有 token、安全与清理契约；2x 屏约减少 75% 预览像素 | 仍有 encode、文件和 decode；需要验证缩放/色彩/几何 | **首选** |
| B. logical-size PNG/WebP 等 encoded preview + 内存二进制通道 | Host 编码后不落盘，main 直接服务 bytes | 去掉 write/read；保留浏览器标准解码 | 当前 JSON Lines 不适合 binary；base64 会膨胀并增加复制；需新增 framing/side channel 和 lease | A 仍不达标时评估 |
| C. logical-size raw RGBA 共享内存 | 不压缩，renderer/importer 直接读 pixels | 无 PNG encode/decode | 2560×1440 仍约 14 MiB；跨进程同步和拷贝复杂；Electron `nativeImage.createFromBitmap` 接收 Buffer 且 `toBitmap` 明示返回 copy，不等价于零拷贝 DOM surface。[Electron `nativeImage`](https://www.electronjs.org/docs/latest/api/native-image) | 通常不优于 A/B |
| D. IOSurface / DXGI shared texture | native GPU tone-map/downscale 到共享 surface，UI 直接合成 | 理论上最低复制与最低 encode latency | Electron 无公开任意 shared texture → DOM image API；需 native addon、Chromium texture/mailbox 集成或改造 Overlay；同步、安全、device loss、HDR/SDR metadata 都要新契约 | 非 MVP，只有明确性能目标驱动时做 |
| E. native Overlay 直接绘制 `CGImage`/D3D surface | 不跨到 Chromium 显示背景 | 使用平台最直接的数据路径；无编码 | Region UI 需要 native 化，或维护 native 背景窗 + Electron 交互窗的对齐、输入、层级；跨平台 UI 重复 | 架构级备选 |
| F. 当前 full-resolution PNG 临时文件 | full-frame conversion + encode + file + decode | 实现、安全边界与调试最简单；无损 | 把 preview 当 artifact；成本随 backing-pixel 数和画面复杂度增长 | 不应作为默认最终方案 |

PNG 仍适合作为 encoded preview 的第一选择：Electron 官方支持 PNG/JPEG，并因透明与无损压缩推荐 PNG。这里应优化的是 **预览尺寸与传输路径**，而不是在没有测量前把文字/UI 截图改成有损格式。[Electron supported image formats](https://www.electronjs.org/docs/latest/api/native-image#supported-formats)

## 5. 推荐的最小演进路径

### 第一步：测清当前分段

在一次 `prepareRegion` 的同一个 correlation/session 上记录：

```text
command received
target resolved
native frame acquired
preview Visual Match/render completed
preview encoded
preview written
Host response received
Overlay document loaded
preview bytes received
image decoded
Overlay shown
```

只需在同一台机器、同一目标显示器上跑少量冷/热样本，并记录 pixel size、HDR state 与 PNG byte size；不需要扩大到全仓性能审计。该测量能区分 acquisition、conversion/encode、文件桥接、BrowserWindow 冷启动和 Chromium decode。

### 第二步：把 preview 降到 target logical size

保持 protocol 的 `targetLogicalSize`、session/token 和 lease 不变；Host 从 authoritative frame 派生逻辑尺寸 SDR preview。macOS 的 Core Image 可直接 render 到指定 bitmap/IOSurface/texture，且 `CIContext` 提供 render 预热能力；Windows 则可在 GPU Visual Match/encode 前增加目标尺寸输出，不需要 CPU 先生成 full RGBA8。[Apple `CIContext`](https://developer.apple.com/documentation/coreimage/cicontext) · [Microsoft screen capture](https://learn.microsoft.com/en-us/windows/apps/develop/media-authoring-processing/screen-capture)

这一改动必须保持：

- preview 与 artifact 来自同一 frozen frame；
- preview 只是视觉引导，不是 artifact truth；
- selection 坐标仍以 target logical space 表达，commit 时由冻结 session 的 scale 映射到原始像素；
- commit 仍对原始 crop 进行正式 Visual Match 与交付；
- 取消、超时和完成继续撤销 token、删除临时文件并释放 native frame。

### 第三步：只在数据证明有必要时替换桥接

如果 logical-size PNG 后，`encode + write/read + decode` 仍是主要瓶颈，再比较：

1. logical-size encoded bytes 的 binary side channel；
2. protocol file/stream response，先减少 main 的整体 `readFile` Buffer；
3. native Overlay 或 shared surface 的架构级方案。

不要把 base64 塞进现有 JSON Lines：它会增加体积和额外编码/复制，也让日志、消息大小与 session 清理边界更难治理。不要为了少一次磁盘读写直接进入 IOSurface/DXGI → Chromium 集成；平台能力存在不代表 Electron 已有可依赖的公开消费接口。[Electron `nativeImage`](https://www.electronjs.org/docs/latest/api/native-image) · [Apple IOSurface](https://developer.apple.com/documentation/iosurface) · [Microsoft shared handle](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgiresource1-createsharedhandle)

## 最终判断

此前根据其他截图工具补上“截图后冻结桌面再框选”是正确的产品修正。需要调整的是实现层对 preview 的定位：**业界可验证的开源实现把冻结帧作为内存位图直接绘制；Lumiere 因 native Host + Electron 边界必须派生跨进程预览，但没有必要把 full-resolution preview 做成正式 PNG artifact。**

对当前架构，logical-size、无损、短生命周期的 encoded preview 是最小充分修改；它保留 HDR authoritative frame、同帧裁剪、Electron 安全 token 和跨平台一致协议，同时直接削减当前最可疑的全帧 conversion/encode/decode 成本。是否继续移除临时文件，应由新增分段计时的数据决定。
