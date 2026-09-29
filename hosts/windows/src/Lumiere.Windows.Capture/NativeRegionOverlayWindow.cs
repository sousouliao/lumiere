using System.Collections.Concurrent;
using System.Runtime.InteropServices;
using Lumiere.Windows.Graphics.Devices;
using Lumiere.Windows.Graphics.Presentation;
using Lumiere.Windows.Interop.Diagnostics;
using Microsoft.Extensions.Logging;

namespace Lumiere.Windows.Capture;

/// <summary>
/// Owns one hidden, reusable Win32 overlay and its message pump. Presentation and input
/// stay on this thread; WGC frame-pool callbacks never enter the window procedure.
/// </summary>
internal sealed class NativeRegionOverlayWindow : INativeRegionOverlay, IDisposable
{
    private const uint WindowStyle = 0x80000000; // WS_POPUP
    private const uint ExtendedStyle = 0x00000008 | 0x00000080; // TOPMOST | TOOLWINDOW
    private const uint WmPaint = 0x000F;
    private const uint WmClose = 0x0010;
    private const uint WmDestroy = 0x0002;
    private const uint WmKeyDown = 0x0100;
    private const uint WmTimer = 0x0113;
    private const uint WmMouseMove = 0x0200;
    private const uint WmLeftDown = 0x0201;
    private const uint WmLeftUp = 0x0202;
    private const uint WmRightDown = 0x0204;
    private const uint WmDisplayChange = 0x007E;
    private const uint WmAppDispatch = 0x8001;
    private const uint WmAppCancel = 0x8002;
    private const uint TimerId = 1;
    private const int EscapeKey = 0x1B;
    private const int HideWindow = 0;
    private const uint PositionWithoutActivation = 0x0010;
    private const uint PositionShowWindow = 0x0040;
    private const uint MonitorDefaultToNull = 0;
    private static readonly ConcurrentDictionary<nint, NativeRegionOverlayWindow> Windows = new();
    private static readonly WindowProcedure Callback = WindowProc;
    private static readonly ILogger Logger = LumiereLoggerFactory.CreateLogger(LogCategories.Capture);

    private readonly GraphicsDeviceResources device;
    private readonly ConcurrentQueue<Action> pending = new();
    private readonly TaskCompletionSource<nint> ready =
        new(TaskCreationOptions.RunContinuationsAsynchronously);
    private readonly Thread thread;
    private readonly string className = $"Lumiere.NativeRegion.{Guid.NewGuid():N}";
    private nint window;
    private NativeRegionPresenter? presenter;
    private NativeRegionSelection? selection;
    private WindowsTargetCapability? target;
    private TaskCompletionSource<NativeRegionOverlayResult>? completion;
    private bool dragging;
    private float pointerX;
    private float pointerY;
    private int nextRequestId;
    private int activeRequestId;
    private int disposed;

    public NativeRegionOverlayWindow(GraphicsDeviceResources device)
    {
        this.device = device ?? throw new ArgumentNullException(nameof(device));
        thread = new Thread(Run)
        {
            IsBackground = true,
            Name = "Lumiere native Region overlay",
        };
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
    }

    public async Task<NativeRegionOverlayResult> SelectAsync(
        CapturedFrameTexture visualMatchSurface,
        WindowsTargetCapability target,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(visualMatchSurface);
        ArgumentNullException.ThrowIfNull(target);
        ObjectDisposedException.ThrowIf(Volatile.Read(ref disposed) != 0, this);
        var hwnd = await ready.Task.WaitAsync(cancellationToken);
        var result = new TaskCompletionSource<NativeRegionOverlayResult>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var requestId = Interlocked.Increment(ref nextRequestId);
        using var registration = cancellationToken.Register(() =>
            _ = PostMessage(hwnd, WmAppCancel, (nuint)requestId, 0));
        Enqueue(hwnd, () => BeginSelection(
            requestId, visualMatchSurface, target, cancellationToken, result));
        return await result.Task;
    }

    private void Run()
    {
        nint hwnd = 0;
        try
        {
            var instance = GetModuleHandle(null);
            var windowClass = new WindowClass
            {
                Size = (uint)Marshal.SizeOf<WindowClass>(),
                Procedure = Callback,
                Instance = instance,
                ClassName = className,
            };
            if (RegisterClassEx(ref windowClass) == 0)
            {
                throw LastError("RegisterClassExW");
            }

            hwnd = CreateWindowEx(
                ExtendedStyle,
                className,
                "Lumiere Region",
                WindowStyle,
                0, 0, 1, 1,
                0, 0, instance, 0);
            if (hwnd == 0)
            {
                throw LastError("CreateWindowExW");
            }

            window = hwnd;
            Windows[hwnd] = this;
            ready.TrySetResult(hwnd);
            while (true)
            {
                var state = GetMessage(out var message, 0, 0, 0);
                if (state == 0) break;
                if (state < 0) throw LastError("GetMessageW");
                _ = TranslateMessage(ref message);
                _ = DispatchMessage(ref message);
            }
        }
        catch (Exception exception)
        {
            Logger.LogError(exception, "operation=NativeRegionOverlay, stage=MessageLoopFailed");
            ready.TrySetException(exception);
            completion?.TrySetException(exception);
        }
        finally
        {
            if (hwnd != 0)
            {
                Windows.TryRemove(hwnd, out _);
            }
            try
            {
                presenter?.Dispose();
            }
            catch (Exception exception)
            {
                Logger.LogError(exception, "operation=NativeRegionOverlay, stage=PresenterDisposeFailed");
            }
            finally
            {
                if (hwnd != 0 && IsWindow(hwnd)) _ = DestroyWindow(hwnd);
                _ = UnregisterClass(className, GetModuleHandle(null));
            }
        }
    }

    private static nint WindowProc(nint hwnd, uint message, nuint wParam, nint lParam)
    {
        if (!Windows.TryGetValue(hwnd, out var overlay))
        {
            return DefWindowProc(hwnd, message, wParam, lParam);
        }

        try
        {
            return overlay.HandleMessage(hwnd, message, wParam, lParam);
        }
        catch (Exception exception)
        {
            Logger.LogError(exception, "operation=NativeRegionOverlay, stage=WindowMessageFailed, message={Message}", message);
            overlay.Fail(exception);
            return 0;
        }
    }

    private nint HandleMessage(nint hwnd, uint message, nuint wParam, nint lParam)
    {
        switch (message)
        {
            case WmAppDispatch:
                while (pending.TryDequeue(out var action)) action();
                return 0;
            case WmAppCancel:
                if (wParam == (nuint)activeRequestId)
                {
                    Logger.LogInformation("operation=NativeRegionOverlay, stage=CancelMessage");
                    Finish(NativeRegionOverlayResult.Cancelled);
                }
                return 0;
            case WmPaint:
                _ = ValidateRect(hwnd, 0);
                if (completion is not null) Draw();
                return 0;
            case WmTimer:
                if (wParam == TimerId) CheckPointerTarget();
                return 0;
            case WmDisplayChange:
                Logger.LogWarning("operation=NativeRegionOverlay, stage=DisplayChanged");
                Finish(NativeRegionOverlayResult.Cancelled);
                return 0;
            case WmKeyDown when wParam == EscapeKey:
            case WmRightDown:
                Logger.LogInformation("operation=NativeRegionOverlay, stage=UserCancelled, message={Message}", message);
                Finish(NativeRegionOverlayResult.Cancelled);
                return 0;
            case WmLeftDown:
                if (selection is not null)
                {
                    UpdatePointer(lParam);
                    selection.Begin(ToLogicalX(pointerX), ToLogicalY(pointerY));
                    dragging = true;
                    _ = SetCapture(hwnd);
                    _ = InvalidateRect(hwnd, 0, false);
                }
                return 0;
            case WmMouseMove:
                if (selection is not null)
                {
                    UpdatePointer(lParam);
                    if (dragging)
                    {
                        selection.Update(ToLogicalX(pointerX), ToLogicalY(pointerY));
                    }
                    _ = InvalidateRect(hwnd, 0, false);
                }
                return 0;
            case WmLeftUp:
                if (selection is not null && dragging)
                {
                    CheckPointerTarget();
                    if (selection is null) return 0;
                    UpdatePointer(lParam);
                    dragging = false;
                    _ = ReleaseCapture();
                    var geometry = selection.Complete(ToLogicalX(pointerX), ToLogicalY(pointerY));
                    if (geometry is not null)
                    {
                        Finish(NativeRegionOverlayResult.Selected(geometry));
                    }
                    else
                    {
                        _ = InvalidateRect(hwnd, 0, false);
                    }
                }
                return 0;
            case WmClose:
                Logger.LogInformation("operation=NativeRegionOverlay, stage=WindowClosed");
                Finish(NativeRegionOverlayResult.Cancelled);
                _ = DestroyWindow(hwnd);
                return 0;
            case WmDestroy:
                PostQuitMessage(0);
                return 0;
            default:
                return DefWindowProc(hwnd, message, wParam, lParam);
        }
    }

    private void BeginSelection(
        int requestId,
        CapturedFrameTexture surface,
        WindowsTargetCapability nextTarget,
        CancellationToken cancellationToken,
        TaskCompletionSource<NativeRegionOverlayResult> result)
    {
        if (cancellationToken.IsCancellationRequested)
        {
            result.TrySetResult(NativeRegionOverlayResult.Cancelled);
            return;
        }
        if (completion is not null)
        {
            result.TrySetException(new InvalidOperationException("The native Region window is already active."));
            return;
        }
        if (nextTarget is not
            {
                SupportsNativeRegionCapture: true, PixelLeft: { } left, PixelTop: { } top,
                PixelWidth: { } width, PixelHeight: { } height, LogicalSize: { } logical
            })
        {
            result.TrySetException(new InvalidOperationException("The target has no physical window bounds."));
            return;
        }

        completion = result;
        activeRequestId = requestId;
        target = nextTarget;
        selection = new NativeRegionSelection(logical);
        pointerX = width / 2f;
        pointerY = height / 2f;
        if (!SetWindowPos(window, new nint(-1), left, top, width, height,
                PositionWithoutActivation))
        {
            throw LastError("SetWindowPos");
        }
        if (presenter is null || presenterSize != (width, height))
        {
            presenter?.Dispose();
            presenter = new NativeRegionPresenter(device, window, width, height);
            presenterSize = (width, height);
        }
        presenter.Attach(surface);
        Draw(); // First matching frame is presented while the window is still hidden.
        if (!SetWindowPos(window, new nint(-1), left, top, width, height,
                PositionShowWindow))
        {
            throw LastError("SetWindowPos(show)");
        }
        if (!IsWindowVisible(window))
        {
            throw new InvalidOperationException("The native Region window did not become visible.");
        }
        Logger.LogInformation("operation=NativeRegionOverlay, stage=Visible, width={Width}, height={Height}",
            width, height);
        if (!ActivateWindow())
        {
            Logger.LogWarning("operation=NativeRegionOverlay, stage=FocusDenied");
        }
        _ = SetTimer(window, TimerId, 50, 0);
    }

    private bool ActivateWindow()
    {
        var foreground = GetForegroundWindow();
        var foregroundThread = foreground == 0
            ? 0
            : GetWindowThreadProcessId(foreground, out _);
        var uiThread = GetCurrentThreadId();
        var attached = foregroundThread != 0
            && foregroundThread != uiThread
            && AttachThreadInput(uiThread, foregroundThread, true);
        try
        {
            var activated = SetForegroundWindow(window);
            _ = SetFocus(window);
            return activated && GetForegroundWindow() == window;
        }
        finally
        {
            if (attached) _ = AttachThreadInput(uiThread, foregroundThread, false);
        }
    }

    private (int Width, int Height) presenterSize;

    private void Draw()
    {
        if (presenter is null || target is null) return;
        NativeRegionPixelRect? pixels = null;
        if (selection?.Current is { } geometry && target.LogicalSize is { } logical)
        {
            var scaleX = target.PixelWidth!.Value / logical.Width;
            var scaleY = target.PixelHeight!.Value / logical.Height;
            pixels = new NativeRegionPixelRect(
                (float)(geometry.X * scaleX),
                (float)(geometry.Y * scaleY),
                (float)((geometry.X + geometry.Width) * scaleX),
                (float)((geometry.Y + geometry.Height) * scaleY));
        }
        presenter.Draw(new NativeRegionDrawing(pixels, pointerX, pointerY));
    }

    private void CheckPointerTarget()
    {
        if (completion is null || target is not
            { PixelLeft: { } left, PixelTop: { } top, PixelWidth: { } width, PixelHeight: { } height })
        {
            return;
        }
        if (!GetCursorPos(out var pointer)) return;
        if (pointer.X >= left && pointer.X < left + width
            && pointer.Y >= top && pointer.Y < top + height)
        {
            return;
        }
        if (MonitorFromPoint(pointer, MonitorDefaultToNull) != 0)
        {
            Finish(NativeRegionOverlayResult.SwitchTarget);
        }
    }

    private void Finish(NativeRegionOverlayResult outcome)
    {
        Complete(outcome, null);
    }

    private void Fail(Exception exception) => Complete(null, exception);

    private void Complete(NativeRegionOverlayResult? outcome, Exception? failure)
    {
        if (completion is not { } result) return;
        if (outcome is not null)
        {
            Logger.LogInformation("operation=NativeRegionOverlay, stage=Finished, action={Action}", outcome.Action);
        }
        try
        {
            _ = KillTimer(window, TimerId);
            if (dragging) _ = ReleaseCapture();
            _ = ShowWindow(window, HideWindow);
            presenter?.Detach();
            if (failure is not null)
            {
                presenter?.Dispose();
                presenter = null;
                presenterSize = default;
            }
        }
        catch (Exception exception)
        {
            Logger.LogError(exception, "operation=NativeRegionOverlay, stage=CleanupFailed");
            failure ??= exception;
        }
        finally
        {
            dragging = false;
            selection = null;
            target = null;
            completion = null;
            activeRequestId = 0;
            if (failure is not null) result.TrySetException(failure);
            else result.TrySetResult(outcome!);
        }
    }

    private void UpdatePointer(nint lParam)
    {
        pointerX = unchecked((short)((long)lParam & 0xFFFF));
        pointerY = unchecked((short)(((long)lParam >> 16) & 0xFFFF));
    }

    private double ToLogicalX(float physical) =>
        physical * target!.LogicalSize!.Width / target.PixelWidth!.Value;

    private double ToLogicalY(float physical) =>
        physical * target!.LogicalSize!.Height / target.PixelHeight!.Value;

    private void Enqueue(nint hwnd, Action action)
    {
        pending.Enqueue(action);
        if (!PostMessage(hwnd, WmAppDispatch, 0, 0))
        {
            throw LastError("PostMessageW");
        }
    }

    public void Dispose()
    {
        if (Interlocked.Exchange(ref disposed, 1) != 0) return;
        try
        {
            var hwnd = ready.Task.GetAwaiter().GetResult();
            _ = PostMessage(hwnd, WmClose, 0, 0);
        }
        catch (Exception) when (ready.Task.IsFaulted) { }
        if (Thread.CurrentThread != thread) thread.Join();
    }

    private static InvalidOperationException LastError(string operation) =>
        new($"{operation} failed with Win32 error {Marshal.GetLastWin32Error()}.");

    private delegate nint WindowProcedure(nint hwnd, uint message, nuint wParam, nint lParam);

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct WindowClass
    {
        public uint Size;
        public uint Style;
        public WindowProcedure Procedure;
        public int ClassExtra;
        public int WindowExtra;
        public nint Instance;
        public nint Icon;
        public nint Cursor;
        public nint Background;
        public string? MenuName;
        public string ClassName;
        public nint IconSmall;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct Point
    {
        public int X;
        public int Y;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct Message
    {
        public nint Window;
        public uint Id;
        public nuint WParam;
        public nint LParam;
        public uint Time;
        public Point Position;
        public uint Private;
    }

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
    private static extern nint GetModuleHandle(string? moduleName);

    [DllImport("user32.dll", EntryPoint = "RegisterClassExW", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern ushort RegisterClassEx(ref WindowClass windowClass);

    [DllImport("user32.dll", EntryPoint = "UnregisterClassW", CharSet = CharSet.Unicode)]
    private static extern bool UnregisterClass(string className, nint instance);

    [DllImport("user32.dll", EntryPoint = "CreateWindowExW", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern nint CreateWindowEx(uint exStyle, string className, string title,
        uint style, int x, int y, int width, int height, nint parent, nint menu,
        nint instance, nint parameter);

    [DllImport("user32.dll", EntryPoint = "DefWindowProcW")]
    private static extern nint DefWindowProc(nint hwnd, uint message, nuint wParam, nint lParam);

    [DllImport("user32.dll", EntryPoint = "GetMessageW")]
    private static extern int GetMessage(out Message message, nint hwnd, uint min, uint max);

    [DllImport("user32.dll")]
    private static extern bool TranslateMessage(ref Message message);

    [DllImport("user32.dll", EntryPoint = "DispatchMessageW")]
    private static extern nint DispatchMessage(ref Message message);

    [DllImport("user32.dll", EntryPoint = "PostMessageW", SetLastError = true)]
    private static extern bool PostMessage(nint hwnd, uint message, nuint wParam, nint lParam);

    [DllImport("user32.dll")]
    private static extern bool DestroyWindow(nint hwnd);

    [DllImport("user32.dll")]
    private static extern bool IsWindow(nint hwnd);

    [DllImport("user32.dll")]
    private static extern bool IsWindowVisible(nint hwnd);

    [DllImport("user32.dll")]
    private static extern void PostQuitMessage(int exitCode);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool SetWindowPos(nint hwnd, nint insertAfter, int x, int y,
        int width, int height, uint flags);

    [DllImport("user32.dll")]
    private static extern bool ShowWindow(nint hwnd, int command);

    [DllImport("user32.dll")]
    private static extern bool SetForegroundWindow(nint hwnd);

    [DllImport("user32.dll")]
    private static extern nint GetForegroundWindow();

    [DllImport("user32.dll")]
    private static extern uint GetWindowThreadProcessId(nint hwnd, out uint processId);

    [DllImport("kernel32.dll")]
    private static extern uint GetCurrentThreadId();

    [DllImport("user32.dll")]
    private static extern bool AttachThreadInput(uint attach, uint to, bool attached);

    [DllImport("user32.dll")]
    private static extern nint SetFocus(nint hwnd);

    [DllImport("user32.dll")]
    private static extern nint SetCapture(nint hwnd);

    [DllImport("user32.dll")]
    private static extern bool ReleaseCapture();

    [DllImport("user32.dll")]
    private static extern nuint SetTimer(nint hwnd, nuint timerId, uint interval, nint callback);

    [DllImport("user32.dll")]
    private static extern bool KillTimer(nint hwnd, nuint timerId);

    [DllImport("user32.dll")]
    private static extern bool InvalidateRect(nint hwnd, nint rectangle, bool erase);

    [DllImport("user32.dll")]
    private static extern bool ValidateRect(nint hwnd, nint rectangle);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool GetCursorPos(out Point point);

    [DllImport("user32.dll")]
    private static extern nint MonitorFromPoint(Point point, uint flags);
}
