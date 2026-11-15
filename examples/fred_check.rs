//! Fetch all FRED indicators with a key: `cargo run --example fred_check -- <api_key>`
fn main() {
    let key = std::env::args().nth(1).unwrap_or_default();
    let client = penguinfin::fred::Client::new(&penguinfin::fred::base_url(), &key);
    let t = std::time::Instant::now();
    let (values, err) = client.fetch_all(chrono::Local::now().date_naive());
    for v in &values {
        println!("{:<38} {:>7.2}%  as of {}", v.indicator.label(), v.value, v.as_of);
    }
    if let Some(e) = err {
        println!("error: {e}");
    }
    println!("took {:?}", t.elapsed());
}
