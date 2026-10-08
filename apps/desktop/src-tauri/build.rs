fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "renderer_ready",
            "get_capture_surface_snapshot",
            "refresh_capture_surface",
            "get_capture_activity",
            "capture_display",
            "capture_region",
            "get_settings_snapshot",
            "choose_save_directory",
            "set_output_delivery",
            "set_after_capture_behavior",
            "set_hdr_status_reminders",
            "set_capture_shortcut",
            "set_shortcut_recording",
            "recover_capture",
            "get_update_snapshot",
            "check_for_updates",
            "download_update",
            "install_update",
        ]),
    ))
    .expect("Tauri build failed")
}
