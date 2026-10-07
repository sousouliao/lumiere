use lumiere_capture_windows::WindowsEngine;
use std::sync::Arc;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    lumiere_windows_host::serve(
        tokio::io::stdin(),
        tokio::io::stdout(),
        Arc::new(WindowsEngine),
    )
    .await
}
