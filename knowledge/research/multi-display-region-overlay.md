# 多显示器 Region Overlay：行业实现与 Lumiere 方案

- 调研日期：2026-09-08
- 范围：Electron 43、macOS AppKit/ScreenCaptureKit、Windows virtual screen/Per-Monitor DPI/WGC，以及 ShareX、ksnip、Flameshot 的官方源码
- 问题：Region Overlay 已显示后，鼠标跨到另一块屏幕时，如何让遮罩、冻结预览与最终裁剪一起切换到目标屏幕

## 结论

“遮罩跟随鼠标跨屏”不能只实现为移动一个窗口。Lumiere 的 Overlay 背景来自 Native Host 持有的 authoritative frozen frame，最终 Region 也从这张帧裁剪；因此一次正确的屏幕切换必须同时切换：

1. native capture target 与 frozen session；
2. renderer 可见的 preview token；
3. Overlay 的 Electron `Display.bounds`；
4. 选区的 target-local logical coordinate space；
5. 旧 session、preview、lease 和迟到异步结果的清理。

行业没有唯一窗口模型。ShareX 和 ksnip 在 Windows 上采用“冻结整个 virtual desktop + 单个巨型选择窗口”；Flameshot 当前源码则在平台间采用全桌面、选屏或当前屏等不同策略。对 Lumiere 而言，长期最稳健的模型是 **每个显示器一张 frozen frame + 每个显示器一个预热 Overlay window**；短期最小充分修改则是 **复用一个 Overlay window，主进程检测鼠标跨屏后串行取消旧 session、prepare 新屏、再原子替换 preview 与 bounds**。

不建议把覆盖整个虚拟桌面的单一 BrowserWindow 作为 Lumiere 默认方案：它与当前 per-display HDR capture target 不吻合，并把 Windows mixed-DPI、macOS Spaces/全屏窗口层级和跨屏坐标拼接同时引入一条路径。

## 1. 当前实现为什么不会跨屏

当前 Region 流程在命令进入时解析指针目标，Native Host 为该目标冻结一帧，Electron 把一个可复用窗口放到该 `display.bounds`。现有正确性模型是“一个 session 对应一块屏幕的一张帧”：

- [`apps/desktop/src/main/index.ts`](../../apps/desktop/src/main/index.ts) 在 `captureRegion` 中 prepare 一次，并在激活前确认指针仍位于相同 Electron Display；
- [`apps/desktop/src/main/region-overlay-controller.ts`](../../apps/desktop/src/main/region-overlay-controller.ts) 的 `activate` 只执行一次 `setBounds` 和一次 snapshot 发送；
- [`apps/desktop/src/main/capture-command-router.ts`](../../apps/desktop/src/main/capture-command-router.ts) 只允许一个 `preparedRegion`；
- macOS `FrozenRegionSession` 和 Windows `FrozenRegionSession` 也都只保存一块目标屏幕的一张 native frame。

Electron `screen` 只有 display added/removed/metrics changed 事件，没有 cursor-crossed-display 事件。[Electron `screen`](https://www.electronjs.org/docs/latest/api/screen) 因此当鼠标离开全屏 Overlay window 后，旧 renderer 也收不到另一屏上的 `pointermove`。如果继续使用单窗，main 必须轮询 `screen.getCursorScreenPoint()` 与 `screen.getDisplayNearestPoint()`，或增加平台原生的全局鼠标观察。

## 2. 业界的三种窗口模型

### 2.1 单窗覆盖整个 virtual desktop

ShareX 当前主线的 `CaptureHelpers.GetScreenBounds()` 返回 `SystemInformation.VirtualScreen`；Region capture 先得到全桌面 bitmap，再将 virtual-screen bounds 交给一个 `RegionCaptureWindow`。[ShareX `CaptureHelpers.cs`](https://github.com/ShareX/ShareX/blob/b3a7f3f8dafcd01d4f1c82432db921a6d20a5147/ShareX.HelpersLib/Helpers/CaptureHelpers.cs#L35-L38) · [`RegionCaptureTasks.cs`](https://github.com/ShareX/ShareX/blob/b3a7f3f8dafcd01d4f1c82432db921a6d20a5147/ShareX.ScreenCaptureLib/RegionCaptureTasks.cs#L81-L111) · [`RegionCaptureWindow.axaml.cs`](https://github.com/ShareX/ShareX/blob/b3a7f3f8dafcd01d4f1c82432db921a6d20a5147/ShareX.ScreenCaptureLib/Presentation/RegionCapture/RegionCaptureWindow.axaml.cs#L1640-L1692)

ksnip 的 Windows wrapper 同样读取 `SM_X/Y/CX/CYVIRTUALSCREEN`，并把 snipping area 设为 virtual desktop；源码为多个缩放不同的屏幕保留了 Qt HiDPI workaround。[ksnip `WinWrapper.cpp`](https://github.com/ksnip/ksnip/blob/343925d24bb0031e636013773fd310e9c56315b9/src/backend/imageGrabber/WinWrapper.cpp#L22-L29) · [`WinSnippingArea.cpp`](https://github.com/ksnip/ksnip/blob/343925d24bb0031e636013773fd310e9c56315b9/src/gui/snippingArea/WinSnippingArea.cpp#L49-L70)

这个模型的优点是一个 renderer 可以持续接收 pointer event，还可以支持跨屏选区。代价是：

- Windows 一个顶层窗口在任一时刻只有一个 effective DPI；跨 mixed-DPI monitor 时要处理 `WM_DPICHANGED` 与 suggested rect。[Microsoft `WM_DPICHANGED`](https://learn.microsoft.com/en-us/windows/win32/hidpi/wm-dpichanged)
- virtual screen 的 origin 可能为负，primary monitor 也不一定处于包围盒左上角。[Microsoft virtual screen](https://learn.microsoft.com/en-us/windows/win32/gdi/the-virtual-screen)
- Lumiere 必须先把多块独立 HDR/SDR capture target 拼成一个 renderer preview，同时仍保留每块屏幕各自的 artifact truth，显著扩大色彩、几何与生命周期边界。

### 2.2 每个显示器一个 Overlay window

Electron 提供 `screen.getAllDisplays()`；每个 `Display.bounds` 都是 DIP，适合直接作为对应 BrowserWindow 的 bounds。[Electron `Display`](https://www.electronjs.org/docs/latest/api/structures/display)

该模型可在截图开始时：

1. 枚举显示器；
2. Native Host 为每块屏幕准备一张 authoritative frozen frame 和 logical-size preview；
3. 为每块屏幕激活一个已经预热的 Overlay；
4. 所有 frame 和 renderer 都 ready 后一起 show；
5. 只有鼠标所在窗口显示 scrim/hint，其余窗口显示未加暗的 frozen preview；
6. `pointerdown` 后锁定 owning display，其他窗口不能提交。

视觉上遮罩随鼠标跨屏，实际上窗口与冻结画面一直覆盖所有屏幕，所以没有切换重抓、旧窗丢事件或无遮挡间隙。每窗天然属于自己的 DPI/Display，负 origin 也直接由 `display.bounds` 表达。

代价是 N 个短期 native frames、previews 和 windows。Lumiere 当前 Host 只允许一个 active frozen Region session，platform-host v3 的 `prepareRegion()` 也没有显式 target，因此这个方案需要真正的多目标 session 协议，不能只在 Electron 层复制窗口。通常显示器数量很少，但 5K HDR frame 的 native 显存/内存仍应测量并设置统一 lease。

### 2.3 复用一个窗口，跨屏时切换 session

main 定时检查 cursor 所在 Electron Display；变化后将 desired display 合并为最新值，并串行执行：

```text
Selecting(old)
  -> Switching(desiredDisplay)
  -> cancel/release old frozen session and preview
  -> prepare native target currently under pointer
  -> validate pointer/display/geometry still stable
  -> grant new preview token
  -> setBounds(newDisplay.bounds) + activate(new generation)
  -> Selecting(new)
```

该模型只保留一张 native frame 和一个 renderer，最符合当前仓库的“single reusable overlay + single frozen session”，是实现当前需求的最小修改。它的固有风险是新屏 prepare 期间的视觉间隙、反复跨屏导致的异步竞态，以及 capture target 在 Shell 与 Host 之间的 TOCTOU。必须使用 `desiredDisplay` coalescing 和单一 switch promise，且每次 activation 都换 generation，拒绝旧 renderer ready/submit 和迟到 prepare。

不应在切换时 destroy/recreate BrowserWindow；应继续复用预热窗口并调用 `setBounds`。

## 3. Display identity：不要跨 API 猜 ID 等值

Electron 只把 `Display.id` 定义为 display 的 unique identifier；官方没有承诺它等于 macOS `CGDirectDisplayID` 或 Windows `HMONITOR`。[Electron `Display`](https://www.electronjs.org/docs/latest/api/structures/display) Electron 唯一明确声明的 ID 对应关系是：`desktopCapturer` 的 `DesktopCapturerSource.display_id` 对应 Electron `screen` 返回的 `Display.id`；这仍然是 Electron 自己两个 API 之间的约定，不是与 Native Host API 的 ABI。[Electron `DesktopCapturerSource`](https://www.electronjs.org/docs/latest/api/structures/desktop-capturer-source)

因此不能把 Electron `Display.id` 强转或序列化成：

- macOS `CGDirectDisplayID`；
- Windows `HMONITOR`；
- Windows display device name。

长期可靠的协议是由 Native Host 签发 opaque `targetToken`：Host 枚举/解析平台目标并把 token、logical size 和必要的 topology generation 返回给 main，`prepareRegion({ targetToken })` 再由同一 Host 精确解析为 `SCDisplay.displayID` 或 `HMONITOR`。Shell 只使用 Electron ID/bounds 管窗口，不解释 native token。

平台 API 本身都天然以单个原生 display/monitor 为目标：ScreenCaptureKit 的 `SCContentFilter` initializer 显式接收一个 `SCDisplay`；Windows WGC interop 的 `CreateForMonitor` 显式接收一个 `HMONITOR`。[Apple `SCContentFilter`](https://developer.apple.com/documentation/screencapturekit/sccontentfilter/init(display:excludingapplications:exceptingwindows:)) · [Microsoft `CreateForMonitor`](https://learn.microsoft.com/en-us/windows/win32/api/windows.graphics.capture.interop/nf-windows-graphics-capture-interop-igraphicscaptureiteminterop-createformonitor)

### 为什么不直接把 Electron cursor point 传给 Host

`screen.getCursorScreenPoint()` 明确返回绝对 DIP，而不是物理像素。[Electron `screen.getCursorScreenPoint`](https://www.electronjs.org/docs/latest/api/screen#screengetcursorscreenpoint) Windows `GetCursorPos`/`MonitorFromPoint` 使用原生 virtual-screen coordinates；mixed-DPI 下不能把 Electron DIP 当成同一种数值直接传入。[Microsoft `GetCursorPos`](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getcursorpos) · [`MonitorFromPoint`](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-monitorfrompoint)

macOS 还存在 Electron/Chromium 屏幕坐标与 AppKit screen coordinates 的 origin/Y 方向转换责任。Native Host 当前使用 `NSEvent.mouseLocation` 与 `NSScreen.frame`，这两者处于同一个 AppKit 坐标域；传入 Electron point 反而会建立一个新的、需要平台验证的坐标合同。[Apple `NSEvent.mouseLocation`](https://developer.apple.com/documentation/appkit/nsevent/mouselocation) · [`NSScreen.frame`](https://developer.apple.com/documentation/appkit/nsscreen/frame)

若暂时不升级协议，较安全的最小策略是：main 只用 Electron API 判断“显示器发生变化”；Host 的现有 `prepareRegion()` 仍在自己的原生坐标域读取 cursor target。Host 返回后，main 再确认 cursor 仍在期望 Electron Display，且 target logical size 与 bounds 匹配，然后才激活。这个校验不能区分相同尺寸的两块屏幕，因此只是过渡方案，不是稳定 identity contract。

## 4. 坐标、DPI 与选区规则

Overlay 内统一使用 Electron DIP / CSS logical coordinates：

```text
localDip.x = globalDip.x - display.bounds.x
localDip.y = globalDip.y - display.bounds.y
```

`Display.bounds` 本身是 DIP；不要先乘主屏 scaleFactor，也不要丢弃负 origin。[Electron `Display.bounds`](https://www.electronjs.org/docs/latest/api/structures/display)

Region commit 继续只提交 target-local logical geometry。Native frozen session 使用自己的 backing dimensions 计算 X/Y 比例并 outward-align 到合法 pixel crop。不要使用“global coordinate × 一个 scaleFactor”，因为：

- 每块屏幕的 scaleFactor 可不同；
- macOS `NSScreen.backingScaleFactor` 是该 screen 每个 screen-space unit 对应的 backing pixels；[Apple `backingScaleFactor`](https://developer.apple.com/documentation/appkit/nsscreen/backingscalefactor)
- Windows per-monitor DPI 会随窗口所在 monitor 改变。[Microsoft High DPI desktop development](https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows)

显示器 added/removed/metrics changed 是 topology invalidation，不是 cursor movement。发生这些事件时，应取消全部当前 Region sessions 并重新枚举/prepare，而不是尝试继续使用缓存的 bounds 或 frame。Apple 也明确指出 `NSScreen.screens` 会随动态配置变化，调用方不应把数组永久缓存。[Apple `NSScreen.screens`](https://developer.apple.com/documentation/appkit/nsscreen/screens)

## 5. Frozen frame 时序

### 长期推荐：所有屏先冻结，再一起显示

```text
enumerate native targets
  -> prepare authoritative frame + logical preview per display
  -> activate/preload overlay per display
  -> wait until all previews decoded
  -> show all windows together
  -> hover changes only active scrim
  -> pointerdown locks one target
  -> commit crop from that target's original frozen frame
  -> dispose every frame/token/window state
```

多块显示器的 capture 完成时间会有细小差异，因此只能承诺“本次 Region 操作开始时为每屏冻结的画面”，不能宣称所有屏幕来自同一物理时刻。确认后仍必须从该屏的同一 frozen frame 裁剪，不能在 pointerdown 时重新 capture，否则会破坏 Lumiere 已有的 same-frame 语义。

### 短期单窗：切换期间必须有明确状态

切换开始后应立即禁止 submit；快速跨屏只更新 `desiredDisplay`，不得并发调用多个 `prepareRegion`。新 preview 已获得且当前 cursor 仍稳定在目标屏后，才更新 active generation。取消、失败、lease timeout、topology change 和应用退出都必须等待/收敛正在运行的 switch，再走同一个幂等清理路径。

对视觉间隙有两种诚实处理：

- 最简单：旧 Overlay 保持显示但不允许框选，待新 frame ready 后一次性迁移；代价是鼠标已在新屏时遮罩短暂留在旧屏；
- 立即隐藏旧 Overlay，prepare 完成后显示新屏；代价是新屏短暂无遮挡。

不能先把旧 frozen preview 搬到新屏再 prepare，因为用户会看到错误屏幕的静止画面。若实测切换延迟或闪烁不可接受，应升级到每屏预冻结/多窗模型，而不是放松 same-frame contract。

## 6. 对 Lumiere 的实施建议

### Phase 1：最小充分修改

保留：

- 一个预热、可复用的 `RegionOverlayController`；
- Host 单 active frozen session；
- logical-size PNG preview token；
- target-local logical selection；
- generation 防旧消息。

增加：

- main-only 的 display watcher，约 100–250 ms 读取一次 Electron cursor display；
- Host-issued opaque target token；Electron 只用自身 display id/bounds 布置窗口；
- `desiredDisplayId`、`activeDisplayId` 和唯一 `switchPromise`；
- 串行 `cancel old -> prepare new -> validate -> grant -> activate`；
- switching 时拒绝 submit，Esc 仍能结束整个操作；
- 快速 A→B→A 往返、prepare 期间取消、同尺寸异缩放屏、负 origin 和 topology change 测试。

这里 100–250 ms 是产品响应与 CPU 唤醒的工程起点，不是平台规定；应以真实双屏观察校准。现有 `CaptureSurfaceMonitor` 已采用 250 ms 轮询 target，可复用其 coalescing 思路，但 Region watcher 应拥有独立、短生命周期的状态，不应把设置页 surface monitor 变成捕获 session 所有者。

### Phase 2：只有实测证明需要时升级

若 Phase 1 在目标机器上出现明显切换空白、闪烁或 capture latency，再引入：

- 每屏 frozen frame 与统一 lease；
- 每屏预热 Overlay window；
- `mouseenter`/sender window identity 驱动 active scrim，pointerdown 锁定 owner。

这一步是协议与资源模型升级，不应和短期修复混在一次“移动窗口”改动里。

## 7. 验证矩阵

自动化至少覆盖：

- A→B 单次切换只产生一个有效新 generation；
- A→B→A 快速往返最终只激活 A，B 的迟到 prepare 被释放；
- switching 时的 submit 被忽略；
- switching 时 Esc 最终释放当前/迟到 session；
- preview grant 或 activation 失败时旧、新 token 都被撤销；
- display-added/removed/metrics-changed 统一终止 session；
- 两块 logical size 相同但 scaleFactor 不同；
- secondary monitor 位于 primary 左侧或上方，bounds 含负坐标。

真实平台观察分别记录：

- macOS：普通双屏、不同 Retina scale、全屏 Space、Screen Recording permission、指针快速跨屏；
- Windows：左右/上下排列、100%+150% mixed DPI、HDR+SDR 混合、负 virtual origin、WGC frame/texture 释放。

macOS 观察不能证明 Windows mixed-DPI 正确，反之亦然。repository unit tests 能证明状态收敛与清理，但不能证明 OS 窗口层级、DPI presentation、真实 capture latency 或 HDR target。

## 最终判断

用户期待的是“鼠标进入哪块屏，哪块屏立即成为可框选的冻结目标”。从产品体验和资源模型看，**每屏预冻结 + 每屏 Overlay，只切换 active scrim** 是最完整方案；从 Lumiere 当前架构和最小修改原则看，**短期先实现串行迁移单窗与 frozen session** 更合理。

无论采用哪一种，必须坚持一个边界：**Overlay window、preview 和 native frozen session 是一个不可拆分的 target bundle。只移动遮罩而不切换 frame/session，会显示错误背景并裁剪错误屏幕。**
