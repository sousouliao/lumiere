using System.Runtime.InteropServices;
using Lumiere.Windows.Graphics.Devices;
using Lumiere.Windows.Graphics.Output;
using Vortice.D3DCompiler;
using Vortice.Direct3D;
using Vortice.Direct3D11;
using Vortice.DXGI;
using Vortice.Mathematics;

namespace Lumiere.Windows.Graphics.Presentation;

/// <summary>
/// Renders the retained WGC texture into a full-size BGRA8 Visual Match surface.
/// This path never maps or encodes the frame before the overlay becomes interactive.
/// </summary>
internal sealed class GpuVisualMatchSurface : IDisposable
{
    private const string ShaderSource = """
        Texture2D<float4> Source : register(t0);
        cbuffer Conversion : register(b0) { float InputLinearScale; float3 Padding; };

        struct VertexOutput { float4 Position : SV_POSITION; };

        VertexOutput VSMain(uint id : SV_VertexID)
        {
            VertexOutput output;
            float2 position = float2((id << 1) & 2, id & 2);
            output.Position = float4(position * float2(2, -2) + float2(-1, 1), 0, 1);
            return output;
        }

        float ToneMap(float value)
        {
            if (value <= 0) return 0;
            if (value <= 1)
            {
                float transition = saturate((value - 0.75) / 0.25);
                transition = transition * transition * (3 - 2 * transition);
                return value * lerp(1, 0.92, transition);
            }
            return 1 - 0.08 / value;
        }

        float ToSrgb(float value)
        {
            return value <= 0.0031308 ? value * 12.92
                : 1.055 * pow(value, 1.0 / 2.4) - 0.055;
        }

        float4 PSMain(VertexOutput input) : SV_TARGET
        {
            float4 sampled = Source.Load(int3(int2(input.Position.xy), 0));
            float3 mapped = float3(
                ToSrgb(ToneMap(sampled.r * InputLinearScale)),
                ToSrgb(ToneMap(sampled.g * InputLinearScale)),
                ToSrgb(ToneMap(sampled.b * InputLinearScale)));
            return float4(saturate(mapped), saturate(sampled.a));
        }
        """;

    private readonly GraphicsDeviceResources device;
    private readonly ID3D11VertexShader vertexShader;
    private readonly ID3D11PixelShader pixelShader;
    private bool disposed;

    public GpuVisualMatchSurface(GraphicsDeviceResources device)
    {
        this.device = device ?? throw new ArgumentNullException(nameof(device));
        using var vertexBytecode = Compile("VSMain", "vs_5_0");
        using var pixelBytecode = Compile("PSMain", "ps_5_0");
        var createdVertexShader = device.Device.CreateVertexShader(vertexBytecode);
        try
        {
            pixelShader = device.Device.CreatePixelShader(pixelBytecode);
            vertexShader = createdVertexShader;
        }
        catch
        {
            createdVertexShader.Dispose();
            throw;
        }
    }

    public CapturedFrameTexture Render(
        CapturedFrameTexture source,
        SrgbVisualMatchConversionContext conversion)
    {
        ObjectDisposedException.ThrowIf(disposed, this);
        ArgumentNullException.ThrowIfNull(source);
        if (source.Texture is null)
        {
            throw new OutputArtifactEncodingException("Frozen Region texture is unavailable.");
        }

        using var sourceView = device.Device.CreateShaderResourceView(source.Texture);
        using var conversionBuffer = device.Device.CreateBuffer(
            new float[] { conversion.InputLinearScale, 0, 0, 0 },
            BindFlags.ConstantBuffer);
        var description = new Texture2DDescription
        {
            Width = (uint)source.Width,
            Height = (uint)source.Height,
            MipLevels = 1,
            ArraySize = 1,
            Format = Format.B8G8R8A8_UNorm,
            SampleDescription = new SampleDescription(1, 0),
            Usage = ResourceUsage.Default,
            BindFlags = BindFlags.RenderTarget | BindFlags.ShaderResource,
            CPUAccessFlags = CpuAccessFlags.None,
            MiscFlags = ResourceOptionFlags.None,
        };
        var target = device.Device.CreateTexture2D(description);
        try
        {
            using var targetView = device.Device.CreateRenderTargetView(target);
            var context = device.ImmediateContext;
            lock (device.ImmediateContextSync)
            {
                try
                {
                    context.OMSetRenderTargets(targetView);
                    context.RSSetViewport(new Viewport(0, 0, source.Width, source.Height));
                    context.IASetPrimitiveTopology(PrimitiveTopology.TriangleList);
                    context.VSSetShader(vertexShader);
                    context.PSSetShader(pixelShader);
                    context.PSSetShaderResource(0, sourceView);
                    context.PSSetConstantBuffer(0, conversionBuffer);
                    context.Draw(3, 0);
                }
                finally
                {
                    context.PSSetShaderResource(0, null!);
                    context.PSSetConstantBuffer(0, null!);
                    context.OMSetRenderTargets((ID3D11RenderTargetView)null!);
                }
            }
            return new CapturedFrameTexture(target, source.Width, source.Height, "GPU Visual Match overlay");
        }
        catch
        {
            target.Dispose();
            throw;
        }
    }

    private static Blob Compile(string entryPoint, string profile)
    {
        var result = Compiler.Compile(
            shaderSource: ShaderSource,
            defines: [],
            include: null!,
            entryPoint: entryPoint,
            sourceName: "GpuVisualMatchSurface.hlsl",
            profile: profile,
            shaderFlags: ShaderFlags.OptimizationLevel3,
            effectFlags: EffectFlags.None,
            out var bytecode,
            out var errors);
        using (errors)
        {
            try
            {
                result.CheckError();
            }
            catch (Exception exception)
            {
                bytecode?.Dispose();
                throw new InvalidOperationException(
                    $"Visual Match {entryPoint} shader compilation failed: "
                    + (errors is null ? "no compiler diagnostic" : Marshal.PtrToStringAnsi(errors.BufferPointer)),
                    exception);
            }
        }
        return bytecode;
    }

    public void Dispose()
    {
        if (disposed) return;
        disposed = true;
        pixelShader.Dispose();
        vertexShader.Dispose();
    }
}
