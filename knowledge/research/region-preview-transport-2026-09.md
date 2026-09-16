# Region 预览传输：Electron 43.4.1 与 eSearch 源码复核

- 日期：2026-09-14
- 范围：预览如何从 native 图像进入 Electron；不包含 Lumiere 本机分段性能实测。
- 版本：Lumiere 的 `apps/desktop/package.json` 固定 Electron `43.4.1`；Electron 证据均固定到该 tag。eSearch 固定到 `dcd572e23440265c0b63afd8f8b0333bc8347030`。
- 证据等级：官方文档、官方测试和源码静态核查；没有执行外部项目或 sharedTexture 原型，因此不宣称性能收益已经验证。

## 结论

**Electron 不要求用 PNG 文件把冻结截图送入界面。** 本项目当前版本已有公开但 Experimental 的 `sharedTexture` API；eSearch 的普通截图预览则直接使用 native addon 返回的 raw pixels 构造 `ImageData` 并画入 Canvas。这两个事实说明当前 encoded preview 是 Lumiere 的实现选择，不能解释为 Electron 的必然限制。它们不能单独证明当前等待的主要时间花在 Electron，也不能证明替换桥接后就能达到某个毫秒目标。[Electron API](https://github.com/electron/electron/blob/v43.4.1/docs/api/shared-texture.md) · [eSearch capture adapter](https://github.com/xushengfeng/eSearch/blob/dcd572e23440265c0b63afd8f8b0333bc8347030/src/renderer/screenShot/screenShot.ts#L299-L315)

旧调研 [`region-preview-pipeline.md`](region-preview-pipeline.md) 的“Electron 无公开任意 shared texture 导入 API”判断，对 **43.4.1 已不成立**。原先要求谨慎管理 native 边界、生命周期、颜色和实测的建议仍成立。不能因 API 存在便把 sharedTexture 当成 JSON Lines 协议上的直接替换件。

## Electron 已提供什么

| 阶段 | 43.4.1 的实际能力与边界 |
| --- | --- |
| main 导入 | `sharedTexture.importSharedTexture({ textureInfo, allReferencesReleased })` 导入 native handle；高层 API 限 main 使用。 |
| main → renderer | `sendSharedTexture({ frame: webContents.mainFrame, importedSharedTexture })` 传送引用；renderer 必须事先注册 receiver，发送有 1000 ms 超时。这个超时是失败上限，不是正常每帧固定延迟。 |
| renderer 消费 | `setSharedTextureReceiver` 收到对象后，`getVideoFrame()` 提供 Web 可消费的 `VideoFrame`。 |
| sandbox | 固定版本 `sandboxed_renderer/api/module-list.ts` 明确列出 `sharedTexture`；官方 managed preload 用 `contextBridge` 暴露回调。无需为主 frame 的这一能力关闭 sandbox 或打开 Node integration。 |
| 输入格式 | 包括 BGRA8、RGBA8、RGBA half-float；另有 YUV 格式。`textureInfo` 可指定 `colorSpace`、尺寸、visible rect、timestamp。格式被列出不等于每个驱动、设备和渲染路径都已验证。 |

来源：[高层 API](https://github.com/electron/electron/blob/v43.4.1/docs/api/shared-texture.md) · [sandbox module list](https://github.com/electron/electron/blob/v43.4.1/lib/sandboxed_renderer/api/module-list.ts) · [managed preload](https://github.com/electron/electron/blob/v43.4.1/spec/fixtures/api/shared-texture/managed/preload.js) · [texture info](https://github.com/electron/electron/blob/v43.4.1/docs/api/structures/shared-texture-import-texture-info.md) · [imported object](https://github.com/electron/electron/blob/v43.4.1/docs/api/structures/shared-texture-imported.md)

官方 fixture 的路径为 OSR texture → main import → send → preload receiver → `getVideoFrame()` → WebGPU render → `frame.close()` → imported release。它证明 Electron 内部的消费链有对应示例；**它的 producer 是 Electron OSR，不是外部 Swift/.NET Host**。[managed renderer](https://github.com/electron/electron/blob/v43.4.1/spec/fixtures/api/shared-texture/managed/renderer.js) · [测试主程序](https://github.com/electron/electron/blob/v43.4.1/spec/api-shared-texture-spec.ts)

## 外部 Host 接入仍须解决什么

1. **handle 必须属于导入进程。** macOS `ioSurface` Buffer 装的是当前进程的 `IOSurfaceRef`，不是任意另一个进程的指针或全局 ID。外部 Host 需要传输 Mach port，再在 main 的 native bridge 获得本地引用。Windows 必须把 NT HANDLE 复制到 main 进程；不能把 .NET Host 的 handle 数字原样塞进 JSON 就视为共享成功。Electron 随后的进程间传输可交给它的 API，但 Host → main 仍是我们要补的边界。[handle 定义](https://github.com/electron/electron/blob/v43.4.1/docs/api/structures/shared-texture-handle.md) · [官方设计说明](https://github.com/electron/electron/blob/v43.4.1/shell/common/api/shared_texture/README.md)
2. **不要提前复用或释放 producer texture。** 文档要求源资源有效至 `allReferencesReleased`；renderer 的 `VideoFrame.close()`、imported 的 `release()` 与 GPU 异步使用并不是同一个时刻。Windows 不接受旧式 non-NT shared handle，因为 Chromium 关闭它会出错。取消、跨屏替换与 renderer 崩溃都要有明确所有权；冻结帧适合使用不再写入的派生 preview surface。[API 生命周期](https://github.com/electron/electron/blob/v43.4.1/docs/api/shared-texture.md) · [设计说明](https://github.com/electron/electron/blob/v43.4.1/shell/common/api/shared_texture/README.md)
3. **不要把 RGBA16F 导入能力等同于 HDR 正确性。** 可指定色彩空间，不代表 Chromium、Canvas/WebGPU、系统 compositor 与显示器自动实现本项目的 sRGB Visual Match 契约。最小可比原型应先导入已完成 Visual Match 的逻辑尺寸 SDR texture，保留 authoritative HDR frame 在 Host。官方测试还允许缩放及色彩处理产生少量差异，不能替代我们的 fidelity 检查。[格式契约](https://github.com/electron/electron/blob/v43.4.1/docs/api/structures/shared-texture-import-texture-info.md) · [测试的色差边界](https://github.com/electron/electron/blob/v43.4.1/spec/api-shared-texture-spec.ts#L214-L221)
4. **Windows 可用性需要自己的验证。** 该 tag 的测试文件在开头只允许 macOS arm64 执行，不能把这份测试当成 Windows 或 Intel Mac 验证。API 仍标为 Experimental；“能绕过 PNG”有文档依据，“端到端零拷贝、所有平台稳定、一定更快”尚无本项目证据。[测试平台 gate](https://github.com/electron/electron/blob/v43.4.1/spec/api-shared-texture-spec.ts#L10-L13) · [Experimental 声明](https://github.com/electron/electron/blob/v43.4.1/docs/api/shared-texture.md)

因此可行的候选链是：

```text
native authoritative frame（保持现有同帧裁剪语义）
  → native 派生逻辑尺寸 SDR texture
  → OS handle IPC + main native bridge
  → sharedTexture.importSharedTexture / sendSharedTexture
  → sandbox preload receiver → VideoFrame → Web 绘制
```

这是架构候选，不是本轮建议直接实施的补丁。应先拿实际分段数据比较节省的 encode/decode/readback 与新增桥接成本。

## eSearch：同样使用 Electron，普通预览不走 PNG 文件

固定源码中的普通 native 截图路径为：

```text
renderer require("node-screenshots")
  → matchedScreen.captureImage()
  → data.toRawSync(true)
  → new Uint8ClampedArray(buffer) → new ImageData(...)
  → canvas.getContext("2d").putImageData(...)
  → clip_show IPC → fullscreen window
```

`screenShot.ts` 的普通 adapter 明确把 `toImageData` 接到 `toRawSync`，而 `toNativeImage` 是另一条显式调用 PNG 编码的分支；`clip_window.ts` 的 `setScreen` 调的是前者。`clip_init` 等待 `setScreen` 完成再发 `clip_show`，故这确实是选区界面出现前使用的数据路径，不是根据 README 猜测。[native 初始化](https://github.com/xushengfeng/eSearch/blob/dcd572e23440265c0b63afd8f8b0333bc8347030/src/renderer/screenShot/screenShot.ts#L45-L57) · [capture / raw 分支](https://github.com/xushengfeng/eSearch/blob/dcd572e23440265c0b63afd8f8b0333bc8347030/src/renderer/screenShot/screenShot.ts#L299-L315) · [ImageData 构造](https://github.com/xushengfeng/eSearch/blob/dcd572e23440265c0b63afd8f8b0333bc8347030/src/renderer/screenShot/screenShot.ts#L449-L453) · [Canvas 背景](https://github.com/xushengfeng/eSearch/blob/dcd572e23440265c0b63afd8f8b0333bc8347030/src/renderer/clip/clip_window.ts#L302-L332) · [显示顺序](https://github.com/xushengfeng/eSearch/blob/dcd572e23440265c0b63afd8f8b0333bc8347030/src/renderer/clip/clip_window.ts#L3750-L3793)

窗口有隐藏创建/复用机制：启用 `keepClip` 时启动即创建隐藏截图窗口，`getClipWin` 复用已经加载的窗口。**这证明可预热，不能证明所有用户配置都预热，也不能直接给出其延迟。**[启动条件](https://github.com/xushengfeng/eSearch/blob/dcd572e23440265c0b63afd8f8b0333bc8347030/src/main/main.ts#L812-L818) · [窗口创建与复用](https://github.com/xushengfeng/eSearch/blob/dcd572e23440265c0b63afd8f8b0333bc8347030/src/main/main.ts#L1032-L1073)

比较时须保留三条边界：

- 普通预览路径仍有 native → JS buffer、typed array、Canvas upload 成本；它不是 GPU 零拷贝。
- eSearch 截图 renderer 开启 `nodeIntegration: true`、关闭 `contextIsolation`，与 Lumiere 的 native Host / sandbox 分工不同。可借鉴省掉编码的思路，不应直接照搬其安全边界。[窗口配置](https://github.com/xushengfeng/eSearch/blob/dcd572e23440265c0b63afd8f8b0333bc8347030/src/main/main.ts#L1064-L1068)
- 自定义截图命令、打开图片、Wayland portal 是其他分支，不能笼统声称 eSearch 所有模式都不使用文件；也未验证其 HDR 输出或与 Lumiere 相同硬件上的延迟。

本次只新增此调研记录，没有修改运行代码或旧决策；旧文中 API 能力判断由本记录明确更正，性能瓶颈仍由 Lumiere 的分段实测认定。
