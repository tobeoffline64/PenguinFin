fn main() {
    // Only the commands listed here can be called from the interface, and only when a
    // capability file allows them (capabilities/default.json).
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
        "boot",
        "start_fresh",
        "pick_import_file",
        "cancel_import",
        "confirm_import",
        "save_settings",
        "set_fred_key",
        "remove_fred_key",
        "save_fred_options",
        "sync_market_data",
        "export_backup",
        "export_for_v2",
        "reset_data",
    ])))
    .expect("tauri build script");
}
