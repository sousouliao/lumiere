use super::{annotations::InkColor, *};
use crate::{
    WindowsEngine,
    interop::{Apartment, DpiScope},
};
use windows::Win32::{Graphics::Direct3D11::*, System::WinRT::RO_INIT_MULTITHREADED};

fn input(x: i16, y: i16) -> LPARAM {
    LPARAM((x as u16 as u32 | ((y as u16 as u32) << 16)) as isize)
}

fn interaction() -> State {
    State {
        active: true,
        width: 1000.0,
        height: 800.0,
        scale: 1.0,
        pointer: (150.0, 150.0),
        mode: Mode::Hover,
        drag_threshold: (4.0, 4.0),
        windows: vec![
            Rect {
                left: 100.0,
                top: 100.0,
                right: 400.0,
                bottom: 300.0,
            },
            Rect {
                left: 0.0,
                top: 0.0,
                right: 800.0,
                bottom: 600.0,
            },
        ],
        locked: None,
        selection: None,
        committed: None,
        drag: None,
        confirming_double_click: false,
        annotations: AnnotationStore::default(),
        toolbar: ToolbarState::default(),
        cached_layout: None,
        cancelled: false,
        dirty: true,
        capture: None,
    }
}

#[test]
fn hover_respects_occlusion_and_click_enters_ready_until_double_click_release() {
    let mut state = interaction();
    let front = state.windows[0];
    assert_eq!(state.rect(), Some(front));
    assert!(state.allows_monitor_switch());

    state.press(false);
    state.pointer_move(input(151, 152), false); // Ordinary click jitter must not become a manual crop.
    state.release();
    assert_eq!(state.mode, Mode::Ready);
    assert_eq!(state.selection, Some(front));
    assert!(state.committed.is_none());
    assert!(!state.allows_monitor_switch());

    state.pointer_move(input(500, 500), false);
    assert_eq!(state.rect(), Some(front));

    state.pointer_move(input(150, 150), false);
    state.double_click(false);
    assert!(state.committed.is_none(), "Wait for the second mouse-up");
    state.release();
    assert_eq!(state.committed, Some(front));

    let mut state = interaction();
    state.windows[0].right = 110.0;
    state.pointer = (105.0, 150.0);
    assert!(
        state.hover().is_none(),
        "A small foreground window blocks the window behind it"
    );
}

#[test]
fn dragging_enters_ready_supports_handles_annotations_and_enter_commit() {
    let mut state = interaction();
    state.press(false);
    state.pointer_move(input(100, 80), false);
    assert_eq!(state.phase(), presenter::Phase::Drag);
    assert!(!state.allows_monitor_switch());
    state.release();
    assert_eq!(state.mode, Mode::Ready);
    assert!(state.committed.is_none());
    let expected = Rect {
        left: 100.0,
        top: 80.0,
        right: 150.0,
        bottom: 150.0,
    };
    assert_eq!(state.selection, Some(expected));

    // Resize bottom-right handle to make room for annotations.
    state.pointer_move(input(150, 150), false);
    state.press(false);
    state.pointer_move(input(420, 340), false);
    state.release();
    assert_eq!(
        state.selection,
        Some(Rect {
            left: 100.0,
            top: 80.0,
            right: 420.0,
            bottom: 340.0,
        })
    );

    // Draw Spotlight (H), Arrow (A), Rect (R), Freehand (P).
    state.key_down(0x48, false, false, false);
    assert_eq!(state.toolbar.tool, Tool::Spotlight);
    state.pointer_move(input(140, 120), false);
    state.press(false);
    state.pointer_move(input(260, 220), false);
    state.release();
    assert_eq!(state.annotations.items.len(), 1);
    assert!(state.annotations.has_spotlights());

    state.key_down(0x41, false, false, false);
    assert_eq!(state.toolbar.tool, Tool::Arrow);
    state.pointer_move(input(280, 140), false);
    state.press(false);
    state.pointer_move(input(360, 200), false);
    state.release();
    assert_eq!(state.annotations.items.len(), 2);

    // Undo / Redo
    state.key_down(0x5A, true, false, false);
    assert_eq!(state.annotations.items.len(), 1);
    state.key_down(0x59, true, false, false);
    assert_eq!(state.annotations.items.len(), 2);

    // Right-click steps drawing tool back to Select rather than discarding annotations.
    state.right_click();
    assert_eq!(state.toolbar.tool, Tool::Select);
    assert!(!state.cancelled);

    // Click outside selection must NOT leak SetCapture.
    state.capture = None;
    state.pointer_move(input(10, 10), false);
    state.press(false);
    assert_ne!(state.capture, Some(true));

    // Selecting an annotation -> Esc clears selection first, second Esc cancels.
    state.annotations.selected_id = Some(state.annotations.items[0].id);
    state.key_down(VK_ESCAPE.0, false, false, false);
    assert!(state.annotations.selected_id.is_none());
    assert!(!state.cancelled);

    // Enter commits cleanly.
    state.key_down(VK_RETURN.0, false, false, false);
    assert_eq!(
        state.committed,
        Some(Rect {
            left: 100.0,
            top: 80.0,
            right: 420.0,
            bottom: 340.0,
        })
    );

    // Small invalid drag returns cleanly to Hover and never commits.
    let mut state = interaction();
    state.press(false);
    state.pointer_move(input(170, 155), false);
    state.release();
    assert_eq!(state.mode, Mode::Hover);
    assert!(state.committed.is_none());
    assert!(state.locked.is_none());
    state.pointer_move(input(1000, 800), false);
    state.double_click(false);
    state.release();
    assert!(
        state.committed.is_none(),
        "Empty space cannot double-click capture"
    );
}

#[test]
fn toolbar_buttons_click_cleanly_without_drawing_behind() {
    let mut state = interaction();
    state.mode = Mode::Ready;
    state.selection = Some(Rect {
        left: 100.0,
        top: 100.0,
        right: 500.0,
        bottom: 360.0,
    });
    state.sync_layout();
    let layout = state.cached_layout.as_ref().unwrap();
    let spotlight_rect = matches!(
        layout.hit_test((
            layout.bar_rect.left + 84.0,
            (layout.bar_rect.top + layout.bar_rect.bottom) * 0.5,
        )),
        Some(ToolbarHit::Tool(Tool::Spotlight))
    );
    assert!(spotlight_rect || layout.bar_rect.width() > 300.0);

    // Clicking same color must not push an empty undo checkpoint.
    let initial_undo = state.annotations.can_undo();
    state.activate_toolbar_hit(ToolbarHit::ColorSwatch(InkColor::Coral));
    assert_eq!(state.annotations.can_undo(), initial_undo);

    // Click Confirm directly via layout hit position.
    state.activate_toolbar_hit(ToolbarHit::Tool(Tool::Rect));
    assert_eq!(state.toolbar.tool, Tool::Rect);
    state.activate_toolbar_hit(ToolbarHit::Confirm);
    assert_eq!(state.committed, state.selection);
}

#[test]
#[ignore = "Shows owned native windows and the real WGC/GPU overlay; run explicitly"]
fn native_window_snap_double_click_crops_the_frozen_frame() {
    use windows::Win32::{Foundation::RECT, Graphics::Dwm::*};
    let _anchor = WindowsEngine::default();
    let _apartment = Apartment::initialize(RO_INIT_MULTITHREADED).unwrap();
    let _dpi = DpiScope::physical_coordinates().unwrap();
    let device = Device::create().unwrap();
    let monitor = crate::interop::cursor_monitor().unwrap();
    let create = |offset: i32| {
        // SAFETY: Fixture owns these top-level STATIC windows on this thread, with RAII destruction.
        let window = Window(unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                w!("STATIC"),
                w!(""),
                WS_OVERLAPPEDWINDOW,
                monitor.bounds.left + offset,
                monitor.bounds.top + offset,
                480,
                320,
                None,
                None,
                None,
                None,
            )
            .unwrap()
        });
        // SAFETY: Show only the fixture-owned HWND without activating another application.
        unsafe {
            let _ = ShowWindow(window.0, SW_SHOWNOACTIVATE);
        }
        window
    };
    let back = create(50);
    let front = create(90);
    let mut visible = RECT::default();
    // SAFETY: Valid owned HWND and writable RECT of the required size.
    unsafe {
        SetWindowPos(
            front.0,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
        .unwrap();
        DwmGetWindowAttribute(
            front.0,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&mut visible as *mut RECT).cast(),
            size_of::<RECT>() as u32,
        )
        .unwrap();
        DwmFlush().unwrap();
    }
    let rect = Rect {
        left: (visible.left - monitor.bounds.left).max(0) as f64,
        top: (visible.top - monitor.bounds.top).max(0) as f64,
        right: (visible.right.min(monitor.bounds.right) - monitor.bounds.left) as f64,
        bottom: (visible.bottom.min(monitor.bounds.bottom) - monitor.bounds.top) as f64,
    };
    let point = (rect.left + 40.0, rect.top + 40.0);
    let hit = |windows: Vec<Rect>| windows.into_iter().find(|r| r.contains(point));
    let snapshot = window_targets::snapshot(HWND::default(), monitor.bounds).unwrap();
    assert_eq!(
        hit(snapshot),
        Some(rect),
        "Frontmost untitled tool window remains eligible"
    );
    assert_ne!(
        hit(window_targets::snapshot(front.0, monitor.bounds).unwrap()),
        Some(rect)
    );
    // SAFETY: Hide only the fixture HWND to verify invisible-window filtering.
    unsafe {
        let _ = ShowWindow(front.0, SW_HIDE);
    }
    assert_ne!(
        hit(window_targets::snapshot(HWND::default(), monitor.bounds).unwrap()),
        Some(rect)
    );
    // SAFETY: Restore the owned fixture, retaining its topmost ordering.
    unsafe {
        let _ = ShowWindow(front.0, SW_SHOWNOACTIVATE);
        DwmFlush().unwrap();
    }

    let mut targets = crate::capture::Targets::default();
    let frame = crate::capture::freeze(&device, &mut targets, &Cancellation::default())
        .unwrap()
        .unwrap();
    let source = device
        .readback(&frame.texture, frame.width, frame.height, None)
        .unwrap();
    let mut overlay = Overlay::new(&frame).unwrap();
    overlay.prepare(&device, &frame).unwrap();
    let position = input(point.0 as i16, point.1 as i16);
    // SAFETY: Synchronous messages exercise the real owned WindowProc; no State borrow spans dispatch.
    unsafe {
        SendMessageW(
            overlay.window.0,
            WM_MOUSEMOVE,
            Some(WPARAM(0)),
            Some(position),
        );
        SendMessageW(
            overlay.window.0,
            WM_LBUTTONDOWN,
            Some(WPARAM(1)),
            Some(position),
        );
        SendMessageW(
            overlay.window.0,
            WM_LBUTTONUP,
            Some(WPARAM(0)),
            Some(position),
        );
    }
    assert_eq!(overlay.state.rect(), Some(rect));
    assert!(overlay.state.committed.is_none());
    // SAFETY: Keep the final mouse-up within the capture overlay, just as physical double-click dispatch does.
    unsafe {
        SendMessageW(
            overlay.window.0,
            WM_LBUTTONDBLCLK,
            Some(WPARAM(1)),
            Some(position),
        );
        assert!(overlay.state.committed.is_none());
        SendMessageW(
            overlay.window.0,
            WM_LBUTTONUP,
            Some(WPARAM(0)),
            Some(position),
        );
    }
    let selection = overlay
        .run(
            &frame,
            &Cancellation::default(),
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
    overlay.finish();
    let Selection::Selected(commit) = selection else {
        panic!("Window selection did not complete");
    };
    let crop = commit.crop;
    assert_eq!(
        Some(crop),
        rect.crop(
            (f64::from(frame.width), f64::from(frame.height)),
            (frame.width, frame.height)
        )
    );
    let cropped = device
        .readback(&frame.texture, frame.width, frame.height, Some(crop))
        .unwrap();
    let expected: Vec<u8> = (crop.y..crop.y + crop.height)
        .flat_map(|y| {
            let start = ((y * frame.width + crop.x) * 8) as usize;
            source[start..start + crop.width as usize * 8]
                .iter()
                .copied()
        })
        .collect();
    assert_eq!(cropped, expected);
    assert!(overlay.state.windows.is_empty());
    assert!(overlay.state.locked.is_none());
    println!(
        "Native top-level snapshot, DWM bounds, click/double-click, same-frame {}x{} crop and cleanup pass",
        crop.width, crop.height
    );
    drop(back);
}

#[test]
#[ignore = "Captures desktop and shows native GPU selection; run explicitly"]
fn retained_gpu_frame_crop_is_exact_after_the_overlay_was_visible() {
    // Keep the process MTA alive while WinRT factory caches/native fixtures are used.
    let _anchor = WindowsEngine::default();
    let _apartment = Apartment::initialize(RO_INIT_MULTITHREADED).unwrap();
    let _dpi = DpiScope::physical_coordinates().unwrap();
    let device = Device::create().unwrap();
    let cancel = Cancellation::default();
    let mut targets = crate::capture::Targets::default();
    let frame = crate::capture::freeze(&device, &mut targets, &cancel)
        .unwrap()
        .unwrap();
    let original = device
        .readback(&frame.texture, frame.width, frame.height, None)
        .unwrap();
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
                        PostMessageW(Some(window), WM_KEYDOWN, WPARAM(0x0D), LPARAM(0)).unwrap();
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
        &frame,
        &Cancellation::default(),
        Instant::now() + Duration::from_secs(15),
    )
    .unwrap();
    controller.join().unwrap();
    overlay.as_mut().unwrap().finish();
    let Selection::Selected(commit) = selected else {
        panic!("Fixture selection was cancelled");
    };
    let crop = commit.crop;
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
