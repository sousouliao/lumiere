using Lumiere.Windows.Graphics.Hdr;
using Vortice.Direct3D;
using Vortice.Direct3D11;
using Vortice.DXGI;

namespace Lumiere.Windows.Graphics.Devices;

internal sealed class GraphicsDeviceResources : IDisposable
{
    private bool disposed;

    public GraphicsDeviceResources(
        ID3D11Device device,
        ID3D11DeviceContext immediateContext,
        IDXGIDevice dxgiDevice,
        string? adapterName,
        FeatureLevel featureLevel,
        EngineReadinessStatus initializationResult)
    {
        Device = device ?? throw new ArgumentNullException(nameof(device));
        ImmediateContext = immediateContext ?? throw new ArgumentNullException(nameof(immediateContext));
        DxgiDevice = dxgiDevice ?? throw new ArgumentNullException(nameof(dxgiDevice));
        AdapterName = string.IsNullOrWhiteSpace(adapterName) ? null : adapterName.Trim();
        FeatureLevel = featureLevel;
        InitializationResult = initializationResult ?? throw new ArgumentNullException(nameof(initializationResult));
    }

    public ID3D11Device Device { get; }

    public ID3D11DeviceContext ImmediateContext { get; }

    // WGC callbacks and the overlay UI use different threads. All immediate-context
    // calls, including a Map/Unmap pair, must be serialized through this gate.
    public object ImmediateContextSync { get; } = new();

    public IDXGIDevice DxgiDevice { get; }

    public string? AdapterName { get; }

    public FeatureLevel FeatureLevel { get; }

    public EngineReadinessStatus InitializationResult { get; }

    public void Dispose()
    {
        if (disposed)
        {
            return;
        }

        disposed = true;
        DxgiDevice.Dispose();
        ImmediateContext.Dispose();
        Device.Dispose();
    }
}
