using System.Text;
using Lumiere.Windows.Capture;
using Microsoft.Extensions.Logging;

namespace Lumiere.Windows.Host;

internal static class Program
{
    public static async Task Main()
    {
        Console.InputEncoding = new UTF8Encoding(encoderShouldEmitUTF8Identifier: false);
        Console.OutputEncoding = new UTF8Encoding(encoderShouldEmitUTF8Identifier: false);
        using var loggerFactory = new StructuredStderrLoggerFactory(Console.Error);
        WindowsDisplayCaptureEngine.ConfigureLogging(loggerFactory);
        ILogger logger = loggerFactory.CreateLogger("Lumiere.Windows.Host.Protocol");
        await using var operations = WindowsHostOperations.CreateDefault(
            loggerFactory.CreateLogger("Lumiere.Windows.Host.Operations"));
        await PlatformHostRequestLoop.RunAsync(Console.In, Console.Out, operations, diagnostic =>
        {
            logger.Log(
                LogLevel.Warning,
                new EventId(0, diagnostic.Event),
                diagnostic,
                exception: null,
                static (state, _) => state.Failure.Message);
        });
    }
}
