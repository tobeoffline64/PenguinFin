//! Fill a database with the sample household: `cargo run --example seed -p penguinfin-store -- /path/to.db`
fn main() {
    let path = std::env::args().nth(1).map(std::path::PathBuf::from).unwrap_or_else(penguinfin_store::db::default_db_path);
    let mut db = penguinfin_store::db::Db::open(&path).expect("open db");
    db.replace_all(&penguinfin_core::sample::sample_data(chrono::Local::now().date_naive())).expect("seed");
    println!("seeded {}", path.display());
}
