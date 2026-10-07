use crate::interop::{Monitor, utf16};
use half::f16;
mod visual_match;
use lumiere_capture_contract::DynamicRange;
pub(crate) use visual_match::VisualMatch;
use windows::{
    Win32::{
        Devices::Display::*,
        Foundation::{E_FAIL, ERROR_INSUFFICIENT_BUFFER, HMODULE},
        Graphics::{
            Direct3D::*,
            Direct3D11::*,
            Dxgi::{Common::*, *},
        },
    },
    core::{Error, Interface, Result},
};

pub(crate) struct Device {
    pub native: ID3D11Device,
    pub context: ID3D11DeviceContext,
}
impl Device {
    pub fn create() -> Result<Self> {
        let (mut device, mut context) = (None, None);
        // SAFETY: Output pointers are valid; hardware device is used with BGRA support.
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&[D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )?;
            let native = device.ok_or_else(|| Error::new(E_FAIL, "No D3D11 device"))?;
            let context = context.ok_or_else(|| Error::new(E_FAIL, "No D3D11 context"))?;
            Ok(Self { native, context })
        }
    }
    pub fn texture(&self, width: u32, height: u32, staging: bool) -> Result<ID3D11Texture2D> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R16G16B16A16_FLOAT,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: if staging {
                D3D11_USAGE_STAGING
            } else {
                D3D11_USAGE_DEFAULT
            },
            BindFlags: if staging {
                0
            } else {
                D3D11_BIND_SHADER_RESOURCE.0 as u32
            },
            CPUAccessFlags: if staging {
                D3D11_CPU_ACCESS_READ.0 as u32
            } else {
                0
            },
            MiscFlags: 0,
        };
        let mut texture = None;
        // SAFETY: Valid description/output pointer; texture has no initial source buffer.
        unsafe {
            self.native
                .CreateTexture2D(&desc, None, Some(&mut texture))?
        };
        texture.ok_or_else(|| Error::new(E_FAIL, "No frame texture"))
    }
    pub fn readback(
        &self,
        texture: &ID3D11Texture2D,
        width: u32,
        height: u32,
        crop: Option<crate::overlay::geometry::Crop>,
    ) -> Result<Vec<u8>> {
        let crop = crop.unwrap_or(crate::overlay::geometry::Crop {
            x: 0,
            y: 0,
            width,
            height,
        });
        if crop.width == 0
            || crop.height == 0
            || crop.x.checked_add(crop.width).is_none_or(|x| x > width)
            || crop.y.checked_add(crop.height).is_none_or(|y| y > height)
        {
            return Err(Error::new(E_FAIL, "Invalid source frame crop"));
        }
        let (width, height) = (crop.width, crop.height);
        let staging = self.texture(width, height, true)?;
        let row_bytes = (width as usize)
            .checked_mul(8)
            .ok_or_else(|| Error::new(E_FAIL, "Frame too large"))?;
        let length = row_bytes
            .checked_mul(height as usize)
            .ok_or_else(|| Error::new(E_FAIL, "Frame too large"))?;
        let mut map = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: Matching textures and exclusive immediate context use; mapped rows live until Unmap.
        unsafe {
            let region = D3D11_BOX {
                left: crop.x,
                top: crop.y,
                front: 0,
                right: crop.x + width,
                bottom: crop.y + height,
                back: 1,
            };
            self.context
                .CopySubresourceRegion(&staging, 0, 0, 0, 0, texture, 0, Some(&region));
            self.context
                .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut map))?;
        }
        let mapped = Mapped {
            context: &self.context,
            texture: &staging,
        };
        if map.pData.is_null() || (map.RowPitch as usize) < row_bytes {
            return Err(Error::new(E_FAIL, "Invalid mapped frame stride"));
        }
        let mut pixels = vec![0; length];
        for y in 0..height as usize {
            // SAFETY: Map succeeded, stride was checked, each source row and destination slice fits.
            unsafe {
                std::ptr::copy_nonoverlapping(
                    (map.pData as *const u8).add(y * map.RowPitch as usize),
                    pixels.as_mut_ptr().add(y * row_bytes),
                    row_bytes,
                );
            }
        }
        drop(mapped);
        Ok(pixels)
    }
}
struct Mapped<'a> {
    context: &'a ID3D11DeviceContext,
    texture: &'a ID3D11Texture2D,
}
impl Drop for Mapped<'_> {
    fn drop(&mut self) {
        // SAFETY: This guard owns one successful Map on subresource zero.
        unsafe { self.context.Unmap(self.texture, 0) };
    }
}

pub(crate) struct DisplayColor {
    pub range: DynamicRange,
    pub scale: f32,
}
pub(crate) fn display_color(monitor: &Monitor) -> Result<DisplayColor> {
    // SAFETY: Factory/output COM lifetimes are owned; enumeration returns borrowed OS monitor IDs.
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1()?;
        let mut adapter_index = 0;
        loop {
            let adapter = match factory.EnumAdapters1(adapter_index) {
                Ok(a) => a,
                Err(e) if e.code() == DXGI_ERROR_NOT_FOUND => break,
                Err(e) => return Err(e),
            };
            let mut output_index = 0;
            loop {
                let output = match adapter.EnumOutputs(output_index) {
                    Ok(o) => o,
                    Err(e) if e.code() == DXGI_ERROR_NOT_FOUND => break,
                    Err(e) => return Err(e),
                };
                if output.GetDesc()?.Monitor == monitor.handle {
                    let desc = output.cast::<IDXGIOutput6>()?.GetDesc1()?;
                    let hdr = matches!(
                        desc.ColorSpace,
                        DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020
                            | DXGI_COLOR_SPACE_YCBCR_STUDIO_G2084_LEFT_P2020
                            | DXGI_COLOR_SPACE_YCBCR_STUDIO_G2084_TOPLEFT_P2020
                            | DXGI_COLOR_SPACE_RGB_STUDIO_G2084_NONE_P2020
                    );
                    return if hdr {
                        let white = sdr_white_level(&monitor.name)?;
                        Ok(DisplayColor {
                            range: DynamicRange::Hdr,
                            scale: 80.0 / white,
                        })
                    } else {
                        Ok(DisplayColor {
                            range: DynamicRange::Sdr,
                            scale: 1.0,
                        })
                    };
                }
                output_index += 1;
            }
            adapter_index += 1;
        }
    }
    Err(Error::new(
        E_FAIL,
        "The capture monitor could not be matched to a DXGI output",
    ))
}
fn sdr_white_level(name: &str) -> Result<f32> {
    // SAFETY: All DisplayConfig buffers/headers are initialized and sized to their native structs.
    unsafe {
        for _ in 0..3 {
            let (mut paths_count, mut modes_count) = (0, 0);
            GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut paths_count, &mut modes_count)
                .ok()?;
            let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); paths_count as usize];
            let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); modes_count as usize];
            let status = QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut paths_count,
                paths.as_mut_ptr(),
                &mut modes_count,
                modes.as_mut_ptr(),
                None,
            );
            if status == ERROR_INSUFFICIENT_BUFFER {
                continue;
            }
            status.ok()?;
            for path in &paths[..paths_count as usize] {
                let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
                    header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                        r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                        size: size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                        adapterId: path.sourceInfo.adapterId,
                        id: path.sourceInfo.id,
                    },
                    ..Default::default()
                };
                if DisplayConfigGetDeviceInfo(&mut source.header) != 0
                    || !utf16(&source.viewGdiDeviceName).eq_ignore_ascii_case(name)
                {
                    continue;
                }
                let mut white = DISPLAYCONFIG_SDR_WHITE_LEVEL {
                    header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                        r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL,
                        size: size_of::<DISPLAYCONFIG_SDR_WHITE_LEVEL>() as u32,
                        adapterId: path.targetInfo.adapterId,
                        id: path.targetInfo.id,
                    },
                    ..Default::default()
                };
                if DisplayConfigGetDeviceInfo(&mut white.header) == 0 && white.SDRWhiteLevel > 0 {
                    return Ok(white.SDRWhiteLevel as f32 / 1000.0 * 80.0);
                }
            }
            break;
        }
    }
    Err(Error::new(
        E_FAIL,
        "HDR target SDR white level is unavailable",
    ))
}

/// Same full-resolution half-rounded conversion as the frozen .NET baseline.
pub(crate) fn rgba16f_to_rgba8(source: &[u8], width: u32, scale: f32) -> Vec<u8> {
    let mut output = Vec::with_capacity(source.len() / 2);
    let width = width as usize;
    let height = source.len() / 8 / width;
    let read = |x: usize, y: usize, channel: usize| {
        let offset = (y * width + x) * 8 + channel * 2;
        f16::from_bits(u16::from_le_bytes([source[offset], source[offset + 1]])).to_f32()
    };
    for pixel in 0..source.len() / 8 {
        let x = pixel % width;
        let y = pixel / width;
        let next_x = (x + 1).min(width - 1);
        let next_y = (y + 1).min(height - 1);
        // Preserve full-resolution bilinear arithmetic, including NaN/Inf behavior.
        let sample = |channel| {
            let a = read(x, y, channel);
            let b = read(next_x, y, channel);
            let c = read(x, next_y, channel);
            let d = read(next_x, next_y, channel);
            let top = a + (b - a) * 0.0;
            let bottom = c + (d - c) * 0.0;
            top + (bottom - top) * 0.0
        };
        for channel in 0..3 {
            output.push(channel_byte(sample(channel), scale));
        }
        output.push(to_byte(sample(3)));
    }
    output
}
fn rounded(value: f32) -> f32 {
    f16::from_f32(value).to_f32()
}
fn channel_byte(linear: f32, scale: f32) -> u8 {
    let f = rounded(linear * scale);
    let tone = rounded(if f <= 0.0 {
        0.0
    } else if f <= 1.0 {
        let t = ((f - 0.75) / 0.25).clamp(0.0, 1.0);
        let smooth = t * t * (3.0 - 2.0 * t);
        f * (1.0 + ((1.0 - 0.08) - 1.0) * smooth)
    } else {
        1.0 - 0.08 / f
    });
    to_byte(rounded(if tone <= 0.0031308 {
        tone * 12.92
    } else {
        1.055 * tone.powf(1.0 / 2.4) - 0.055
    }))
}
fn to_byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(serde::Deserialize)]
    struct Fixture {
        cases: Vec<Case>,
    }
    #[derive(serde::Deserialize)]
    struct Case {
        scale: f32,
        bits: Vec<u16>,
        rgba8: Vec<u8>,
    }
    #[test]
    fn conversion_matches_unmodified_dotnet_baseline_bytes() {
        let fixture: Fixture =
            serde_json::from_str(include_str!("../tests/fixtures/visual-match-baseline.json"))
                .unwrap();
        for case in fixture.cases {
            let source: Vec<u8> = case.bits.into_iter().flat_map(u16::to_le_bytes).collect();
            let actual = rgba16f_to_rgba8(&source, (source.len() / 8) as u32, case.scale);
            assert_eq!(actual.len(), case.rgba8.len());
            for (index, (actual, expected)) in actual.iter().zip(&case.rgba8).enumerate() {
                assert_eq!(actual, expected, "scale {}, byte {}", case.scale, index);
            }
        }
    }
}
