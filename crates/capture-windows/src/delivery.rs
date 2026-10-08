use crate::Cancellation;
use lumiere_capture_contract::{
    CaptureParams, DeliveryOutcome, DeliveryResult, DeliveryTarget, Failure, FailureCode,
};
use std::{
    fs::OpenOptions,
    io::{self, Write},
    path::{Path, PathBuf},
};
use windows::Win32::{
    Foundation::SYSTEMTIME,
    System::SystemInformation::GetLocalTime,
    UI::Shell::{FOLDERID_Pictures, KF_FLAG_DEFAULT, SHGetKnownFolderPath},
};

pub(crate) fn encode_png(
    width: u32,
    height: u32,
    pixels: &[u8],
) -> Result<Vec<u8>, png::EncodingError> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(pixels)?;
        writer.finish()?;
    }
    Ok(bytes)
}
pub(crate) fn deliver(
    params: &CaptureParams,
    png: &[u8],
    cancel: &Cancellation,
) -> Vec<DeliveryResult> {
    params
        .delivery
        .targets()
        .iter()
        .map(|&target| {
            let result = if cancel.is_cancelled() {
                Err("Capture cancelled before delivery".to_owned())
            } else {
                match target {
                    DeliveryTarget::Clipboard => clipboard(png).map(|_| None),
                    DeliveryTarget::Folder => save_directory(params).and_then(|folder| {
                        // The baseline attempts to create both the configured and default folders.
                        let _ = std::fs::create_dir_all(&folder);
                        save(&folder, &timestamp_name(), png)
                            .map(|p| Some(p.to_string_lossy().into_owned()))
                            .map_err(|e| e.to_string())
                    }),
                }
            };
            DeliveryResult {
                target,
                outcome: match result {
                    Ok(file_path) => DeliveryOutcome::Success { file_path },
                    Err(message) => DeliveryOutcome::Failed {
                        failure: Failure {
                            code: FailureCode::DeliveryFailed,
                            message,
                            retryable: true,
                        },
                    },
                },
            }
        })
        .collect()
}
fn save_directory(params: &CaptureParams) -> Result<PathBuf, String> {
    if let Some(folder) = &params.save_directory {
        return Ok(PathBuf::from(folder.trim()));
    }
    // SAFETY: The returned string is owned by the shell allocator and freed after copying.
    let pictures = unsafe {
        SHGetKnownFolderPath(&FOLDERID_Pictures, KF_FLAG_DEFAULT, None).and_then(|pointer| {
            let copied = pointer.to_string();
            windows::Win32::System::Com::CoTaskMemFree(Some(pointer.0.cast()));
            Ok(copied?)
        })
    };
    let pictures = match pictures {
        Ok(path) if !path.is_empty() => PathBuf::from(path),
        _ => std::env::var_os("USERPROFILE")
            .map(|p| PathBuf::from(p).join("Pictures"))
            .ok_or_else(|| "The Windows user profile could not be resolved".to_owned())?,
    };
    Ok(pictures.join("Lumiere"))
}
fn timestamp_name() -> String {
    // SAFETY: GetLocalTime returns a value with no external pointers or owned handles.
    let time: SYSTEMTIME = unsafe { GetLocalTime() };
    format!(
        "Lumiere-{:04}-{:02}-{:02}-{:02}{:02}{:02}",
        time.wYear, time.wMonth, time.wDay, time.wHour, time.wMinute, time.wSecond
    )
}
fn save(folder: &Path, name: &str, bytes: &[u8]) -> io::Result<PathBuf> {
    if !folder.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Save folder is not available",
        ));
    }
    let mut suffix = 1u32;
    loop {
        let path = folder.join(if suffix == 1 {
            format!("{name}.png")
        } else {
            format!("{name}-{suffix}.png")
        });
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if let Err(error) = file.write_all(bytes) {
                    drop(file);
                    let _ = std::fs::remove_file(&path);
                    return Err(error);
                }
                return Ok(path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                suffix += 1;
            }
            Err(error) => return Err(error),
        }
    }
}
mod clipboard;
#[cfg(test)]
pub(crate) use clipboard::read_png as read_clipboard_png;
fn clipboard(png: &[u8]) -> Result<(), String> {
    clipboard::write(png)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn png_roundtrips_rgba_and_srgb_metadata() {
        let pixels = [10, 20, 30, 255, 250, 150, 50, 128];
        let bytes = encode_png(2, 1, &pixels).unwrap();
        let mut reader = png::Decoder::new(io::Cursor::new(bytes))
            .read_info()
            .unwrap();
        assert!(reader.info().srgb.is_some());
        let mut decoded = vec![0; reader.output_buffer_size().unwrap()];
        let frame = reader.next_frame(&mut decoded).unwrap();
        assert_eq!((frame.width, frame.height), (2, 1));
        assert_eq!(frame.color_type, png::ColorType::Rgba);
        assert_eq!(decoded, pixels);
    }
    #[test]
    fn folder_delivery_preserves_collision_and_rejects_missing_directory() {
        let folder =
            std::env::temp_dir().join(format!("lumiere-rust-delivery-{}", std::process::id()));
        std::fs::create_dir(&folder).unwrap();
        let first = save(&folder, "Lumiere-fixture", b"first").unwrap();
        let second = save(&folder, "Lumiere-fixture", b"second").unwrap();
        assert_eq!(std::fs::read(first).unwrap(), b"first");
        assert!(second.ends_with("Lumiere-fixture-2.png"));
        assert!(save(&folder.join("missing"), "Lumiere-fixture", b"x").is_err());
        std::fs::remove_dir_all(folder).unwrap();
    }
}
