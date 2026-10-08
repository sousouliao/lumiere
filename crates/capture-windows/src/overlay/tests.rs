use super::*;
use crate::{
    WindowsEngine,
    interop::{Apartment, DpiScope},
};
use windows::Win32::{Graphics::Direct3D11::*, System::WinRT::RO_INIT_MULTITHREADED};

#[test]
#[ignore = "Captures desktop and shows native GPU selection; run explicitly"]
fn retained_gpu_frame_crop_is_exact_after_the_overlay_was_visible() {
    // Keep the process MTA alive while WinRT factory caches/native fixtures are used.
    let _anchor = WindowsEngine::default();
    let _apartment = Apartment::initialize(RO_INIT_MULTITHREADED).unwrap();
    let _dpi = DpiScope::physical_coordinates().unwrap();
    let device = Device::create().unwrap();
    let cancel = Cancellation::default();
    let frame = crate::capture::freeze(&device, &cancel).unwrap().unwrap();
    let original = device
        .readback(&frame.texture, frame.width, frame.height, None)
        .unwrap();
    let shader = VisualMatch::new(&device).unwrap();
    let controller = std::thread::spawn(move || {
        let _dpi = DpiScope::physical_coordinates().unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            // SAFETY: Discover only this fixture process's visible selection window.
            unsafe {
                if let Ok(window) = FindWindowW(None, w!("Lumiere Region")) {
                    let mut pid = 0;
                    GetWindowThreadProcessId(window, Some(&mut pid));
                    if pid == std::process::id() && IsWindowVisible(window).as_bool() {
                        let pack = |x: isize, y: isize| LPARAM(x | y << 16);
                        // Reverse direction also exercises the native drag normalization.
                        PostMessageW(Some(window), WM_LBUTTONDOWN, WPARAM(1), pack(300, 240))
                            .unwrap();
                        PostMessageW(Some(window), WM_MOUSEMOVE, WPARAM(1), pack(100, 100))
                            .unwrap();
                        PostMessageW(Some(window), WM_LBUTTONUP, WPARAM(0), pack(100, 100))
                            .unwrap();
                        break;
                    }
                }
            }
            assert!(Instant::now() < deadline, "No visible native overlay");
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    let mut overlay = None;
    let selected = select(
        &mut overlay,
        &device,
        &shader,
        &frame,
        &Cancellation::default(),
        Instant::now() + Duration::from_secs(15),
    )
    .unwrap();
    controller.join().unwrap();
    let Selection::Selected(crop) = selected else {
        panic!("Fixture selection was cancelled");
    };
    assert_eq!(
        crop,
        Crop {
            x: 100,
            y: 100,
            width: 200,
            height: 140
        }
    );
    let cropped = device
        .readback(&frame.texture, frame.width, frame.height, Some(crop))
        .unwrap();
    let expected: Vec<u8> = (crop.y..crop.y + crop.height)
        .flat_map(|y| {
            let start = ((y * frame.width + crop.x) * 8) as usize;
            original[start..start + (crop.width * 8) as usize]
                .iter()
                .copied()
        })
        .collect();
    assert_eq!(
        cropped, expected,
        "Native selection must crop the original retained RGBA16F frame"
    );
    assert!(
        device
            .readback(
                &frame.texture,
                frame.width,
                frame.height,
                Some(Crop {
                    x: frame.width,
                    y: 0,
                    width: 1,
                    height: 1
                })
            )
            .is_err()
    );

    // The retained source descriptor also remains unchanged after native presentation.
    let mut description = D3D11_TEXTURE2D_DESC::default();
    // SAFETY: Texture remains owned, and the descriptor output is valid.
    unsafe {
        frame.texture.GetDesc(&mut description);
    }
    assert_eq!(description.Width, frame.width);
    println!(
        "same-frame crop {}x{} at {},{} matches retained source bytes exactly",
        crop.width, crop.height, crop.x, crop.y
    );
}
