# Windows 截图窗口与元素自动框选调研

- 调研日期：2026-10-10。
- 范围：悬停自动框选、双击确认、Windows 窗口和控件识别；不实现新功能。
- 本文区分源码证据、API 能力与 Lumiere 方案建议；覆盖率和延迟尚未在本机实测。

## 结论

建议采用 Win32/DWM 整窗与子窗口识别，加 UI Automation（UIA）细粒度候选的分层方案。
默认悬停选整窗，显式进入元素模式或切换层级；双击确认同一个已锁定候选。
“较为完全”需要不同信息源互补，“精准”需要坐标、遮挡、候选层级和冻结时序共同正确。
仅使用 `WindowFromPoint` 或全桌面 UIA `ElementFromPoint` 不足以完成 Lumiere 的需求。

不能承诺任意应用的所有可见元素都能识别。未提供无障碍信息的自绘区域缺少语义边界；
识别失败应回退整窗或手动框选，而不是生成貌似精准的错误矩形。

## 已确认的业界实现

### ShareX：窗口矩形快照与本地点命中

审阅源码基于 `develop`；查阅时分支 HEAD 为 `7d8e364b3086514aa6599bab8e5b1d466294e97f`。

- `WindowsRectangleList` 使用 `EnumWindows`，可通过 `EnumChildWindows` 收集原生子窗口。
- 排除自身 Overlay、不可见/cloaked 窗口及特定辅助窗口；子窗口矩形裁剪到父范围。
- `CaptureHelpers.GetWindowRectangle` 优先 DWM Extended Frame Bounds，失败回退 `GetWindowRect`。
- Region UI 异步收集矩形，在本地按鼠标坐标命中；单击形成候选选区，拖动转换为手动选区，支持双击确认。
- 此路径里的“控件”是 HWND 子窗口，不等于任意现代应用的完整语义控件树。

来源：[窗口枚举](https://github.com/ShareX/ShareX/blob/7d8e364b3086514aa6599bab8e5b1d466294e97f/ShareX.ScreenCaptureLib/Helpers/WindowsRectangleList.cs)、[边界读取](https://github.com/ShareX/ShareX/blob/7d8e364b3086514aa6599bab8e5b1d466294e97f/ShareX.HelpersLib/Helpers/CaptureHelpers.cs)、[Region 交互](https://github.com/ShareX/ShareX/blob/7d8e364b3086514aa6599bab8e5b1d466294e97f/ShareX.ScreenCaptureLib/Presentation/RegionCapture/RegionCaptureWindow.axaml.cs)。

### Greenshot：窗口与递归子窗口命中

`WindowDetails` 枚举窗口、过滤不可见和最小化目标、读取 DWM 可见边界，并通过
`FindChildUnderPoint` 递归命中子窗口。它证明传统截图工具可以依靠 OS 几何实现稳定吸附，
也说明 HWND 层级本身不能覆盖所有自绘控件。

来源：[Greenshot WindowDetails](https://github.com/greenshot/greenshot/blob/main/src/Greenshot.Base/Core/WindowDetails.cs)。

### Snipaste 与 QQ：产品行为与内部算法分开

Snipaste 官网明确列出自动 UI 元素检测，其官方更新记录也记载过元素检测导致无响应的修复。
本轮未取得其内部实现证据，不能断言采用某个具体算法，更不能把第三方同名下载站当官方技术说明。
QQ 的行为由用户观察提供，本轮未取得可验证的内部实现资料。

来源：[Snipaste 官网](https://www.snipaste.com/)、[官方更新记录](https://www.snipaste.com/download.html)。

## 信息源选择

| 信息源 | 能提供什么 | 边界与建议 |
| --- | --- | --- |
| Win32 + DWM | 顶层窗口、弹窗、原生子窗口和可见外边界 | 整窗主路径；自绘控件通常没有独立 HWND |
| UI Automation | 应用暴露的按钮、输入框、面板、列表项等语义和矩形 | 细粒度主要补充；依赖 provider 质量、权限和响应速度 |
| MSAA / IAccessible | 老应用的无障碍对象 | UIA 已有互操作桥接；先测实际缺口，再决定是否直接补充 |
| 图像边缘或模型检测 | 无语义树区域的视觉候选 | 缺少确定的语义、层级和边界；不作为首版默认路径 |

UIA 支持按桌面点读取元素以及缓存属性，但元素矩形可能包括被遮挡或不可点击的区域。
MSAA 与 UIA 能互操作，仍不完全等价。Electron 官方也说明其无障碍树有启用机制，
不能以“使用某个框架”推断所有应用的内部元素一定可读取。

来源：[UIA 点查询与缓存](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomation-elementfrompointbuildcache)、[矩形语义](https://learn.microsoft.com/en-us/dotnet/api/system.windows.automation.automationelement.boundingrectangleproperty?view=windowsdesktop-10.0)、[UIA/MSAA 互操作](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-msaa)、[Electron 无障碍](https://github.com/electron/electron/blob/main/docs/tutorial/accessibility.md)。

## Lumiere 的关键设计约束

### 1. 绕开自己的截图窗口

当前 `overlay.rs` 创建 `WS_EX_TOPMOST | WS_EX_TOOLWINDOW` 的全屏原生窗口。
它必须接收鼠标，不能为了识别而让整个截图窗口永久穿透，也不能逐帧隐藏导致闪烁。

建议在显示 Overlay 前，收集底层窗口及 Z 序和必要的原生子窗口矩形，明确排除截图辅助 HWND。
在本地先命中最上层真实窗口，再在该窗口内选择子候选。
不要简单按标题非空或排除全部 tool window，否则可能漏掉有意义的菜单和浮动面板。

细粒度识别采用已知目标 HWND 的 UIA 根：通过 `ElementFromHandle` 取得目标，
进行有界子树读取与矩形命中，避免把全桌面 `ElementFromPoint` 当作可忽略 Overlay 的接口。
UIA 客户端没有这里假设的“任意指定根节点的 ElementFromPoint”捷径。
窗口内重叠元素的优先级和 provider 行为仍需样本验证；矩形包含关系并不等于视觉命中。

来源：[ElementFromHandle](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomation-elementfromhandle)。

### 2. 识别几何与冻结帧一致

当前 `lib.rs` 先 `capture::freeze`，再显示 Overlay；选中后裁剪同一张 RGBA16F texture。
应用未被暂停，UIA 读取的是实时界面。WGC 和 UIA 不提供这里所需的跨 API 原子快照。

建议在冻结附近采集窗口边界，检测采集前后明显的几何变化；会话保存不可变的候选快照。
UIA 缓存以同一冻结会话为单位，避免沿用上一次截图的矩形。
Overlay 显示后补充的 UIA 结果只能视为实时探测；目标移动、布局变化或时序不可信时，
拒绝细粒度候选，回退原整窗快照或手动选区。窗口边界稳定也不能证明内部布局完全稳定。

预采整个桌面的完整 UIA 树会拖慢截图；纯按需实时读取又可能偏离冻结画面。
推荐原型比较“当前目标的有界预采”和“按需补充并做失效校验”，再按实际结果确定预算。
这项权衡需要明确接受，不能用缓存一词宣称彻底解决。

### 3. 像素几何与遮挡

- 顶层边界优先 `DWMWA_EXTENDED_FRAME_BOUNDS`，避免不可见 resize border 被框进去。
- UIA 矩形是物理屏幕坐标；沿用 `DpiScope` 的 Per-Monitor V2，转换到冻结显示器局部坐标。
- 支持负桌面坐标；禁止再盲目乘 DPI；浮点边界采用明确且统一的像素取整与裁剪规则。
- 先确定鼠标位置的最上层真实窗口，再找其元素，避免吸到后面被遮挡窗口的按钮。
- 非矩形窗口、透明区域与圆角需要明确 hit-test/region 策略；DWM 外框不是透明轮廓。
- 现有帧只属于一块显示器，跨屏窗口只能裁剪当前帧覆盖的部分；完整跨屏截图属于另一项能力。

自动框选仍然是桌面冻结图上的矩形裁剪。被其他窗口遮住的内容不会自动恢复，
圆角矩形四角也可能包含桌面背景；独立窗口捕获和透明输出不包含在本方案中。

来源：[GetWindowRect 与 DWM 边界](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowrect)、[UIA 物理坐标](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-screenscaling)。

### 4. 稳定交互

建议默认悬停框整窗，显式元素模式提供控件 → 容器 → 整窗候选链；合并重复矩形，
排除空、越界、隐藏和无意义的布局候选。层级切换交互需实现前确认，不默认扩展设置页。

单击锁定当前候选，第二击按系统双击判定完成；移动超过系统拖动阈值后转为手动框选。
沿用拖拽释放截图与 Esc/右键取消。锁定后异步识别不能替换候选。
需要注册 `CS_DBLCLKS` 并正确处理 `WM_LBUTTONDBLCLK` 及随后的 mouse-up，避免重复提交。

来源：[Windows 鼠标与双击消息](https://learn.microsoft.com/en-us/windows/win32/inputdev/about-mouse-input)。

### 5. 不让目标应用拖住截图

微软建议桌面 UIA 调用放在不拥有窗口的独立 MTA 线程。
缓存必要几何与类型属性，限制树深、节点数及每会话预算；只处理最新指针请求，
带会话/请求编号拒绝迟到结果。主交互线程只消费普通数据，不传递 COM 对象。

`IUIAutomation2` 提供 Connection/Transaction timeout；默认 transaction timeout 为 20 秒，
不适合截图悬停热路径。应用侧放弃等待不等于取消底层 COM 调用；预算不能冒充硬中断。
先以一个有界 worker 验证超时与清理；若实测 provider 卡住且必须保证回收，再考虑进程隔离，
不要一开始引入复杂的多进程探测架构或为每次鼠标移动创建线程。

来源：[线程要求](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-threading)、[Transaction timeout](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomation2-put_transactiontimeout)、[Connection timeout](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomation2-put_connectiontimeout)。

普通权限 UIA 对提权应用存在访问限制；UIAccess 有签名、安装位置与用途要求。
不能为追求覆盖率默认提权整个 Lumiere，也不能把 UIAccess 当作通用绕过方案。

来源：[微软 UIA 安全边界](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-securityoverview)。

## 实现前需要验证的样本

以原生 HWND 应用、WPF/WinUI、Chromium/Electron、Qt 和自绘画布为样本类别，
纳入资源管理器、浏览器、VS Code、QQ/微信及一个 Qt 应用；框架名称不构成通过证据。
覆盖菜单、浮动窗口、最大化、部分遮挡、混合 DPI、负坐标、跨屏窗口和提权目标。

分别记录整窗命中正确率、可读取元素的命中正确率、不可读取类别、像素边界误差、
候选抖动、启动与悬停延迟 p50/p95，以及超时后拖拽和取消是否可用。
精度应对照真实可见边界，不能仅以“等于 provider 返回的矩形”作为精准证明。
小范围原型先暴露实际缺口，再决定 MSAA 直连、应用适配或图像候选是否值得增加。

本轮只有源码/API 调研，没有执行竞品运行对比或识别覆盖率测试。本文不更改契约、
CURRENT 或当前截图实现；后续实现应由一个带明确验收条件的 Issue 管理。
