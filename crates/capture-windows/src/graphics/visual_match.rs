use super::*;
use windows::{
    Win32::Graphics::Direct3D::Fxc::*,
    core::{PCSTR, s},
};

pub(crate) struct VisualMatch {
    vertex: ID3D11VertexShader,
    pixel: ID3D11PixelShader,
}
impl VisualMatch {
    pub fn new(device: &Device) -> Result<Self> {
        let vertex_code = compile(s!("VSMain"), s!("vs_5_0"))?;
        let pixel_code = compile(s!("PSMain"), s!("ps_5_0"))?;
        let (mut vertex, mut pixel) = (None, None);
        // SAFETY: Compiled blobs remain alive for the creation calls; no class linkage is used.
        unsafe {
            device
                .native
                .CreateVertexShader(blob_bytes(&vertex_code), None, Some(&mut vertex))?;
            device
                .native
                .CreatePixelShader(blob_bytes(&pixel_code), None, Some(&mut pixel))?;
        }
        Ok(Self {
            vertex: vertex.ok_or_else(|| Error::new(E_FAIL, "No vertex shader"))?,
            pixel: pixel.ok_or_else(|| Error::new(E_FAIL, "No pixel shader"))?,
        })
    }
    pub fn render(
        &self,
        device: &Device,
        source: &ID3D11Texture2D,
        width: u32,
        height: u32,
        scale: f32,
    ) -> Result<ID3D11Texture2D> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_RENDER_TARGET | D3D11_BIND_SHADER_RESOURCE).0 as u32,
            ..Default::default()
        };
        let (mut texture, mut target, mut view, mut buffer) = (None, None, None, None);
        let data = [scale, 0.0, 0.0, 0.0];
        let buffer_desc = D3D11_BUFFER_DESC {
            ByteWidth: 16,
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            ..Default::default()
        };
        let initial = D3D11_SUBRESOURCE_DATA {
            pSysMem: data.as_ptr().cast(),
            ..Default::default()
        };
        // SAFETY: All descriptions/buffers live through creation; immediate context is exclusively worker-owned.
        unsafe {
            device
                .native
                .CreateTexture2D(&desc, None, Some(&mut texture))?;
            let texture = texture.ok_or_else(|| Error::new(E_FAIL, "No visual-match texture"))?;
            device
                .native
                .CreateRenderTargetView(&texture, None, Some(&mut target))?;
            device
                .native
                .CreateShaderResourceView(source, None, Some(&mut view))?;
            device
                .native
                .CreateBuffer(&buffer_desc, Some(&initial), Some(&mut buffer))?;
            let context = &device.context;
            context.OMSetRenderTargets(Some(&[target]), None);
            context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                Width: width as f32,
                Height: height as f32,
                MaxDepth: 1.0,
                ..Default::default()
            }]));
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            context.VSSetShader(&self.vertex, None);
            context.PSSetShader(&self.pixel, None);
            context.PSSetShaderResources(0, Some(&[view]));
            context.PSSetConstantBuffers(0, Some(&[buffer]));
            context.Draw(3, 0);
            context.PSSetShaderResources(0, Some(&[None]));
            context.PSSetConstantBuffers(0, Some(&[None]));
            context.OMSetRenderTargets(Some(&[None]), None);
            Ok(texture)
        }
    }
}
fn compile(entry: PCSTR, profile: PCSTR) -> Result<ID3DBlob> {
    let source = include_bytes!("visual-match.hlsl");
    let (mut code, mut errors) = (None, None);
    // SAFETY: Source and entry/profile strings are valid through compilation; output blobs are owned.
    unsafe {
        if let Err(error) = D3DCompile(
            source.as_ptr().cast(),
            source.len(),
            s!("visual-match.hlsl"),
            None,
            None,
            entry,
            profile,
            D3DCOMPILE_OPTIMIZATION_LEVEL3,
            0,
            &mut code,
            Some(&mut errors),
        ) {
            let diagnostic = errors
                .as_ref()
                .map(|b| String::from_utf8_lossy(blob_bytes(b)).into_owned())
                .unwrap_or_default();
            return Err(Error::new(error.code(), diagnostic));
        }
    }
    code.ok_or_else(|| Error::new(E_FAIL, "Shader compiler returned no bytecode"))
}
fn blob_bytes(blob: &ID3DBlob) -> &[u8] {
    // SAFETY: Blob owns its immutable byte buffer for the returned borrow's lifetime.
    unsafe { std::slice::from_raw_parts(blob.GetBufferPointer().cast(), blob.GetBufferSize()) }
}
