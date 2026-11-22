//! PenguinFin 2's backend. It alone opens the database and the network; the interface (Leptos,
//! in the webview) asks it through the commands below, which capabilities/default.json allows.

mod secrets;

use penguinfin_core::fred::{self, FredConfig};
use penguinfin_core::ipc::{Boot, BootStatus, DbInfo, FredOptions, ImportPreview, KeyStatus, Snapshot, SyncOutcome};
use penguinfin_core::model::{AppData, Settings};
use penguinfin_store::db::{Db, default_db_path, export_json};
use penguinfin_store::export::{self, Imported, Source};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

type Result<T> = std::result::Result<T, String>;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

struct AppState {
    db_path: PathBuf,
    db: Mutex<Option<Db>>,
    boot: Mutex<BootStatus>,
    pending_import: Mutex<Option<Imported>>,
    syncing: AtomicBool,
}

fn today() -> chrono::NaiveDate {
    chrono::Local::now().date_naive()
}

fn db_path() -> PathBuf {
    // PENGUINFIN_DB points at another file (testing). Otherwise the same place 1.x used, so an
    // existing install upgrades in place.
    std::env::var_os("PENGUINFIN_DB").map(PathBuf::from).unwrap_or_else(default_db_path)
}

/// Open (and if needed upgrade) the database, and move a 1.x FRED key into the keychain.
fn open_existing(path: &Path) -> (BootStatus, Option<Db>) {
    match Db::open_upgrading(path) {
        Ok((mut db, copy)) => {
            if let Ok(cfg) = db.fred_config()
                && !cfg.api_key.trim().is_empty()
                && secrets::set(cfg.api_key.trim()).is_ok()
            {
                let _ = db.clear_fred_key();
            }
            let _ = db.set_meta("app_version", APP_VERSION);
            // A new month means new planned payments for debt buckets (as 1.x does at start).
            let _ = db.sync_debt_buckets();
            (BootStatus::Ready { upgraded_backup: copy.map(|p| p.display().to_string()) }, Some(db))
        }
        Err(error) => (BootStatus::Failed { error }, None),
    }
}

fn snapshot(db: &Db) -> Result<Snapshot> {
    let mut data = db.load()?;
    let mut fred = data.fred.clone();
    fred.api_key.clear();
    data.fred.api_key.clear();
    let market = data.market.clone();
    let fred_key = match secrets::get() {
        Some((k, place)) => KeyStatus { set: true, tail: k.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect(), stored_in: place.into() },
        None => KeyStatus::default(),
    };
    let db_info = DbInfo {
        path: db.path().display().to_string(),
        schema_version: db.schema_version()?,
        migrated_from: db.meta("migrated_from")?.and_then(|v| v.parse().ok()),
    };
    Ok(Snapshot { data, fred, market, fred_key, db: db_info })
}

/// Run a write, keep debt buckets in step (as 1.x does after every change), and return the new snapshot.
fn mutate(state: &AppState, f: impl FnOnce(&mut Db) -> Result<()>) -> Result<Snapshot> {
    let mut guard = state.db.lock().map_err(|_| "the database is busy")?;
    let db = guard.as_mut().ok_or("no database is open")?;
    f(db)?;
    db.sync_debt_buckets()?;
    snapshot(db)
}

/// Match native popups (select lists) and the title bar to the app's own theme.
fn apply_theme(app: &AppHandle, dark: bool) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_theme(Some(if dark { tauri::Theme::Dark } else { tauri::Theme::Light }));
    }
}

#[tauri::command(async)]
fn boot(app: AppHandle, state: State<'_, AppState>) -> Result<Boot> {
    let status = state.boot.lock().map_err(|_| "busy")?.clone();
    let snap = match state.db.lock().map_err(|_| "busy")?.as_ref() {
        Some(db) => Some(snapshot(db)?),
        None => None,
    };
    if let Some(s) = &snap {
        apply_theme(&app, s.data.settings.dark_mode);
    }
    Ok(Boot {
        status,
        snapshot: snap,
        db_path: state.db_path.display().to_string(),
        today: today().format("%Y-%m-%d").to_string(),
        platform: std::env::consts::OS.to_string(),
        form: if cfg!(any(target_os = "android", target_os = "ios")) { "phone" } else { "desktop" }.into(),
        app_version: APP_VERSION.into(),
    })
}

/// Welcome screen: create the database, empty or with the sample household.
#[tauri::command(async)]
fn start_fresh(kind: String, state: State<'_, AppState>) -> Result<Snapshot> {
    let mut guard = state.db.lock().map_err(|_| "busy")?;
    if guard.is_none() {
        let mut db = Db::open(&state.db_path)?;
        db.set_meta("app_version", APP_VERSION)?;
        if kind == "sample" {
            db.replace_all(&penguinfin_core::sample::sample_data(today()))?;
        }
        *guard = Some(db);
        *state.boot.lock().map_err(|_| "busy")? = BootStatus::Ready { upgraded_backup: None };
    }
    snapshot(guard.as_ref().expect("just opened"))
}

fn describe(found: &Imported) -> String {
    match &found.source {
        Source::Export(h) => format!("a PenguinFin export from version {}", h.app_version),
        Source::JsonBackup => "a PenguinFin backup".into(),
        Source::Database { .. } => "a PenguinFin database".into(),
    }
}

/// Ask for a file (export, backup or database), read it, and describe it. Nothing changes yet.
#[tauri::command(async)]
fn pick_import_file(app: AppHandle, state: State<'_, AppState>) -> Result<Option<ImportPreview>> {
    let Some(picked) = app
        .dialog()
        .file()
        .set_title("Import PenguinFin data")
        .add_filter("PenguinFin files", &["pfexport", "json", "db"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    let found = export::read_file(&path)?;
    let d = &found.data;
    let preview = ImportPreview {
        file_name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        kind: describe(&found),
        summary: export::summary(&found.counts()),
        net_worth: d.net_worth(),
        total_debt: d.total_debt(),
        currency: d.settings.currency_symbol.clone(),
    };
    *state.pending_import.lock().map_err(|_| "busy")? = Some(found);
    Ok(Some(preview))
}

#[tauri::command(async)]
fn cancel_import(state: State<'_, AppState>) -> Result<()> {
    *state.pending_import.lock().map_err(|_| "busy")? = None;
    Ok(())
}

/// Replace all data with the file read by `pick_import_file`. Existing data is first copied
/// beside the database as `penguinfin-before-import-<time>.db`.
#[tauri::command(async)]
fn confirm_import(state: State<'_, AppState>) -> Result<Snapshot> {
    let found = state.pending_import.lock().map_err(|_| "busy")?.take().ok_or("choose a file to import first")?;
    let mut guard = state.db.lock().map_err(|_| "busy")?;
    if let Some(db) = guard.as_ref() {
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        db.backup_to(&state.db_path.with_file_name(format!("penguinfin-before-import-{stamp}.db")))?;
    } else {
        let db = Db::open(&state.db_path)?;
        db.set_meta("app_version", APP_VERSION)?;
        *guard = Some(db);
        *state.boot.lock().map_err(|_| "busy")? = BootStatus::Ready { upgraded_backup: None };
    }
    let db = guard.as_mut().expect("open");
    let expected = (export::Counts::of(&found.data), found.data.net_worth());
    db.replace_all(&found.data)?;
    db.sync_debt_buckets()?;
    // Check the result against the file before saying it worked.
    let now = db.load()?;
    if export::Counts::of(&now) != expected.0 || (now.net_worth() - expected.1).abs() > 0.005 {
        return Err("the imported data doesn't match the file; your previous data was saved beside the database".into());
    }
    snapshot(db)
}

#[tauri::command(async)]
fn save_settings(settings: Settings, app: AppHandle, state: State<'_, AppState>) -> Result<Snapshot> {
    let dark = settings.dark_mode;
    let snap = mutate(&state, |db| db.save_settings(&settings))?;
    apply_theme(&app, dark);
    Ok(snap)
}

#[tauri::command(async)]
fn set_fred_key(key: String, state: State<'_, AppState>) -> Result<Snapshot> {
    let key = key.trim();
    if !fred::valid_api_key(key) {
        return Err("A FRED API key is 32 lowercase letters and digits.".into());
    }
    secrets::set(key)?;
    mutate(&state, |db| db.clear_fred_key())
}

#[tauri::command(async)]
fn remove_fred_key(state: State<'_, AppState>) -> Result<Snapshot> {
    secrets::remove();
    mutate(&state, |db| db.clear_fred_key())
}

#[tauri::command(async)]
fn save_fred_options(options: FredOptions, state: State<'_, AppState>) -> Result<Snapshot> {
    mutate(&state, |db| {
        db.save_fred_config(&FredConfig {
            api_key: String::new(),
            sync_on_launch: options.sync_on_launch,
            inflation_source: options.inflation_source,
        })?;
        db.clear_fred_key()?;
        db.apply_market_inflation()
    })
}

#[tauri::command]
async fn sync_market_data(state: State<'_, AppState>) -> Result<SyncOutcome> {
    let Some((key, _)) = secrets::get() else { return Err("Add your FRED API key first.".into()) };
    if state.syncing.swap(true, Ordering::SeqCst) {
        return Err("A sync is already running.".into());
    }
    let fetched = tauri::async_runtime::spawn_blocking(move || {
        fred::Client::new(&fred::base_url(), &key).fetch_all(today())
    })
    .await;
    state.syncing.store(false, Ordering::SeqCst);
    let (values, error) = fetched.map_err(|e| e.to_string())?;
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
    let snapshot = mutate(&state, |db| {
        db.save_market_data(&values, &now, error.as_deref())?;
        db.apply_market_inflation()
    })?;
    let (message, error) = match (&error, values.is_empty()) {
        (None, _) => (format!("Market data updated ({} indicators)", values.len()), false),
        (Some(e), true) => (format!("FRED sync failed: {e}"), true),
        (Some(e), false) => (format!("Updated {} indicators; some failed: {e}", values.len()), true),
    };
    Ok(SyncOutcome { snapshot, message, error })
}

fn downloads(app: &AppHandle) -> PathBuf {
    app.path().download_dir().unwrap_or_else(|_| penguinfin_store::db::home_dir())
}

fn ask_save_path(app: &AppHandle, name: String, ext: &str, label: &str) -> Option<PathBuf> {
    app.dialog()
        .file()
        .set_title("Save a copy of your data")
        .set_directory(downloads(app))
        .set_file_name(name)
        .add_filter(label, &[ext])
        .blocking_save_file()
        .and_then(|p| p.into_path().ok())
}

fn current_data(state: &AppState) -> Result<AppData> {
    let guard = state.db.lock().map_err(|_| "busy")?;
    let mut d = guard.as_ref().ok_or("no database is open")?.load()?;
    d.fred.api_key.clear();
    Ok(d)
}

/// Save a JSON backup where the person chooses. Returns the path, or None if they cancelled.
#[tauri::command(async)]
fn export_backup(app: AppHandle, state: State<'_, AppState>) -> Result<Option<String>> {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let Some(path) = ask_save_path(&app, format!("penguinfin-backup-{stamp}.json"), "json", "PenguinFin backup") else {
        return Ok(None);
    };
    export_json(&current_data(&state)?, &path)?;
    Ok(Some(path.display().to_string()))
}

/// Save a checked `.pfexport` file for another computer or phone.
#[tauri::command(async)]
fn export_for_v2(app: AppHandle, state: State<'_, AppState>) -> Result<Option<String>> {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let name = format!("penguinfin-{stamp}.{}", export::EXTENSION);
    let Some(path) = ask_save_path(&app, name, export::EXTENSION, "PenguinFin export") else { return Ok(None) };
    export::write_export(&current_data(&state)?, &path, APP_VERSION)?;
    Ok(Some(path.display().to_string()))
}

/// "sample" replaces everything with the sample household; "empty" erases everything.
/// Settings are kept either way.
#[tauri::command(async)]
fn reset_data(kind: String, state: State<'_, AppState>) -> Result<Snapshot> {
    mutate(&state, |db| {
        let settings = db.settings()?;
        let mut d = if kind == "sample" { penguinfin_core::sample::sample_data(today()) } else { AppData::default() };
        d.settings = settings;
        db.replace_all(&d)
    })
}

pub fn run() {
    let path = db_path();
    let (status, db) = if path.exists() { open_existing(&path) } else { (BootStatus::Welcome, None) };
    let state = AppState {
        db_path: path,
        db: Mutex::new(db),
        boot: Mutex::new(status),
        pending_import: Mutex::new(None),
        syncing: AtomicBool::new(false),
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            boot,
            start_fresh,
            pick_import_file,
            cancel_import,
            confirm_import,
            save_settings,
            set_fred_key,
            remove_fred_key,
            save_fred_options,
            sync_market_data,
            export_backup,
            export_for_v2,
            reset_data,
        ])
        .run(tauri::generate_context!())
        .expect("PenguinFin could not start");
}
