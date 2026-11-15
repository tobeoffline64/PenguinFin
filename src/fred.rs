//! FRED (Federal Reserve Economic Data) client.
//!
//! API: <https://fred.stlouisfed.org/docs/api/fred/series_observations.html>
//! `GET /fred/series/observations?series_id=..&api_key=..&file_type=json`
//! - `api_key` is a free 32-character lowercase alphanumeric key.
//! - `sort_order=desc&limit=1` returns just the newest observation.
//! - `units=pc1` returns "percent change from a year ago" (used for CPI inflation).
//! - Missing values are reported as `"."`.
//! - Errors come back as `{"error_code": 400, "error_message": "..."}`; limit is 120 req/min.
//!
//! A sync makes 7 small requests in parallel on worker threads, with short timeouts.
//! Nothing here runs on the UI thread.

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const DEFAULT_BASE_URL: &str = "https://api.stlouisfed.org";
pub const KEY_SIGNUP_URL: &str = "https://fredaccount.stlouisfed.org/apikeys";

/// The economic indicators PenguinFin uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Indicator {
    /// Market-implied average inflation over the next 10 years (T10YIE).
    ExpectedInflation10y,
    /// CPI inflation over the past 12 months (CPIAUCSL, percent change from a year ago).
    CpiInflation,
    /// Effective federal funds rate (DFF); savings APYs track it.
    FedFunds,
    /// 10-year Treasury yield (DGS10).
    Treasury10y,
    /// Average 30-year fixed mortgage rate (MORTGAGE30US).
    Mortgage30y,
    /// S&P 500 annualized price return over the last 10 years (SP500; excludes dividends).
    Sp500Return10y,
}

impl Indicator {
    pub const ALL: [Indicator; 6] = [
        Indicator::ExpectedInflation10y,
        Indicator::CpiInflation,
        Indicator::FedFunds,
        Indicator::Treasury10y,
        Indicator::Mortgage30y,
        Indicator::Sp500Return10y,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Indicator::ExpectedInflation10y => "expected_inflation_10y",
            Indicator::CpiInflation => "cpi_inflation",
            Indicator::FedFunds => "fed_funds",
            Indicator::Treasury10y => "treasury_10y",
            Indicator::Mortgage30y => "mortgage_30y",
            Indicator::Sp500Return10y => "sp500_return_10y",
        }
    }

    pub fn from_key(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|i| i.key() == s)
    }

    pub fn series_id(self) -> &'static str {
        match self {
            Indicator::ExpectedInflation10y => "T10YIE",
            Indicator::CpiInflation => "CPIAUCSL",
            Indicator::FedFunds => "DFF",
            Indicator::Treasury10y => "DGS10",
            Indicator::Mortgage30y => "MORTGAGE30US",
            Indicator::Sp500Return10y => "SP500",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Indicator::ExpectedInflation10y => "Expected inflation, next 10 years",
            Indicator::CpiInflation => "Inflation, last 12 months (CPI)",
            Indicator::FedFunds => "Federal funds rate",
            Indicator::Treasury10y => "10-year Treasury yield",
            Indicator::Mortgage30y => "30-year mortgage rate",
            Indicator::Sp500Return10y => "S&P 500, 10-year average return",
        }
    }

    pub fn note(self) -> &'static str {
        match self {
            Indicator::ExpectedInflation10y => "Market-implied (10-year breakeven). Good long-run assumption.",
            Indicator::CpiInflation => "Consumer prices vs. a year ago.",
            Indicator::FedFunds => "High-yield savings usually pay a little below this.",
            Indicator::Treasury10y => "A low-risk return benchmark.",
            Indicator::Mortgage30y => "National weekly average for a 30-year fixed loan.",
            Indicator::Sp500Return10y => "Annualized price change; dividends add roughly 1–2% more.",
        }
    }
}

/// A fetched value. All values are percentages (e.g. 2.3 means 2.3%).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketValue {
    pub indicator: Indicator,
    pub value: f64,
    /// Date of the observation the value is based on.
    pub as_of: NaiveDate,
}

/// Which FRED number (if any) drives the inflation assumption.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum InflationSource {
    /// Use the inflation typed in Settings.
    #[default]
    Manual,
    ExpectedInflation10y,
    CpiInflation,
}

impl InflationSource {
    pub const ALL: [InflationSource; 3] =
        [InflationSource::Manual, InflationSource::ExpectedInflation10y, InflationSource::CpiInflation];

    pub fn key(self) -> &'static str {
        match self {
            InflationSource::Manual => "manual",
            InflationSource::ExpectedInflation10y => "expected_10y",
            InflationSource::CpiInflation => "cpi",
        }
    }
    pub fn from_key(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.key() == s)
    }
    pub fn label(self) -> &'static str {
        match self {
            InflationSource::Manual => "Enter it myself",
            InflationSource::ExpectedInflation10y => "FRED: expected inflation, next 10 years (recommended)",
            InflationSource::CpiInflation => "FRED: inflation over the last 12 months",
        }
    }
    pub fn indicator(self) -> Option<Indicator> {
        match self {
            InflationSource::Manual => None,
            InflationSource::ExpectedInflation10y => Some(Indicator::ExpectedInflation10y),
            InflationSource::CpiInflation => Some(Indicator::CpiInflation),
        }
    }
}

/// FRED settings. Stored locally; deliberately not part of JSON backups (the key is a secret).
#[derive(Debug, Clone, PartialEq)]
pub struct FredConfig {
    pub api_key: String,
    pub sync_on_launch: bool,
    pub inflation_source: InflationSource,
}

impl Default for FredConfig {
    fn default() -> Self {
        FredConfig { api_key: String::new(), sync_on_launch: true, inflation_source: InflationSource::Manual }
    }
}

impl FredConfig {
    pub fn enabled(&self) -> bool {
        !self.api_key.trim().is_empty()
    }
}

/// Cached market data plus the outcome of the last sync.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MarketData {
    pub values: Vec<MarketValue>,
    /// Local time of the last sync attempt, "YYYY-MM-DD HH:MM".
    pub last_sync: Option<String>,
    /// Error from the last sync, if it failed (fully or partly).
    pub last_error: Option<String>,
}

impl MarketData {
    pub fn get(&self, i: Indicator) -> Option<&MarketValue> {
        self.values.iter().find(|v| v.indicator == i)
    }
}

/// A 32-character lowercase alphanumeric string, per the FRED docs.
pub fn valid_api_key(key: &str) -> bool {
    key.len() == 32 && key.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

// ---------- HTTP + parsing ----------

#[derive(Deserialize)]
struct ObservationsResponse {
    observations: Vec<Observation>,
}

#[derive(Deserialize)]
struct Observation {
    date: String,
    value: String,
}

#[derive(Deserialize)]
struct ErrorResponse {
    error_message: String,
}

/// Parse an observations response into (date, value) pairs, skipping missing values (".").
pub fn parse_observations(body: &str) -> Result<Vec<(NaiveDate, f64)>, String> {
    if let Ok(err) = serde_json::from_str::<ErrorResponse>(body) {
        return Err(clean_error(&err.error_message));
    }
    let resp: ObservationsResponse =
        serde_json::from_str(body).map_err(|e| format!("Unexpected response from FRED: {e}"))?;
    Ok(resp
        .observations
        .into_iter()
        .filter_map(|o| {
            let date = NaiveDate::parse_from_str(&o.date, "%Y-%m-%d").ok()?;
            let value = o.value.trim().parse::<f64>().ok().filter(|v| v.is_finite())?;
            Some((date, value))
        })
        .collect())
}

fn clean_error(msg: &str) -> String {
    let m = msg.split("  ").collect::<Vec<_>>().join(" ");
    if m.contains("api_key") {
        format!("FRED rejected the API key: {}", m.trim())
    } else {
        m.trim().to_string()
    }
}

/// Minimal FRED client. `base_url` is configurable so tests can point it at a local server.
pub struct Client {
    agent: ureq::Agent,
    base_url: String,
    api_key: String,
}

impl Client {
    pub fn new(base_url: &str, api_key: &str) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(5))
            .timeout(Duration::from_secs(12))
            .user_agent(concat!("PenguinFin/", env!("CARGO_PKG_VERSION")))
            .build();
        Client { agent, base_url: base_url.trim_end_matches('/').to_string(), api_key: api_key.trim().to_string() }
    }

    /// Fetch observations. `params` are extra query parameters (e.g. sort_order, limit).
    fn observations(&self, series_id: &str, params: &[(&str, &str)]) -> Result<Vec<(NaiveDate, f64)>, String> {
        let mut req = self
            .agent
            .get(&format!("{}/fred/series/observations", self.base_url))
            .query("series_id", series_id)
            .query("api_key", &self.api_key)
            .query("file_type", "json");
        for (k, v) in params {
            req = req.query(k, v);
        }
        let body = match req.call() {
            Ok(resp) => resp.into_string().map_err(|e| format!("Reading FRED response failed: {e}"))?,
            // FRED reports problems (bad key, unknown series) with a JSON body on 4xx/5xx.
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                return Err(parse_observations(&body).err().unwrap_or_else(|| format!("FRED returned HTTP {code}")));
            }
            Err(ureq::Error::Transport(t)) => return Err(format!("Couldn't reach FRED: {t}")),
        };
        parse_observations(&body)
    }

    /// The most recent non-missing observation (looks back a little in case the newest is ".").
    fn latest(&self, series_id: &str, units: &str) -> Result<(NaiveDate, f64), String> {
        self.observations(series_id, &[("sort_order", "desc"), ("limit", "10"), ("units", units)])?
            .into_iter()
            .next()
            .ok_or_else(|| format!("FRED has no recent data for {series_id}"))
    }

    /// The first non-missing observation on or after `start`.
    fn first_since(&self, series_id: &str, start: NaiveDate) -> Result<(NaiveDate, f64), String> {
        let start = start.format("%Y-%m-%d").to_string();
        self.observations(series_id, &[("sort_order", "asc"), ("limit", "10"), ("observation_start", start.as_str())])?
            .into_iter()
            .next()
            .ok_or_else(|| format!("FRED has no data for {series_id} since {start}"))
    }

    pub fn fetch(&self, indicator: Indicator, today: NaiveDate) -> Result<MarketValue, String> {
        let id = indicator.series_id();
        let (as_of, value) = match indicator {
            Indicator::CpiInflation => self.latest(id, "pc1")?,
            Indicator::Sp500Return10y => {
                let ten_years_ago = today.with_year(today.year() - 10).unwrap_or(today);
                let (d0, v0) = self.first_since(id, ten_years_ago)?;
                let (d1, v1) = self.latest(id, "lin")?;
                (d1, annualized_return(d0, v0, d1, v1).ok_or("Not enough S&P 500 history on FRED")?)
            }
            _ => self.latest(id, "lin")?,
        };
        Ok(MarketValue { indicator, value, as_of })
    }

    /// Fetch every indicator in parallel. Returns what succeeded plus a combined error
    /// message for anything that failed.
    pub fn fetch_all(&self, today: NaiveDate) -> (Vec<MarketValue>, Option<String>) {
        let results: Vec<Result<MarketValue, String>> = std::thread::scope(|s| {
            let handles: Vec<_> = Indicator::ALL.iter().map(|&i| s.spawn(move || self.fetch(i, today))).collect();
            handles.into_iter().map(|h| h.join().unwrap_or_else(|_| Err("FRED request crashed".into()))).collect()
        });
        let mut values = Vec::new();
        let mut errors: Vec<String> = Vec::new();
        for r in results {
            match r {
                Ok(v) => values.push(v),
                Err(e) if !errors.contains(&e) => errors.push(e),
                Err(_) => {}
            }
        }
        let error = (!errors.is_empty()).then(|| errors.join(" · "));
        (values, error)
    }
}

/// Compound annual growth rate between two observations, in percent.
pub fn annualized_return(d0: NaiveDate, v0: f64, d1: NaiveDate, v1: f64) -> Option<f64> {
    let years = (d1 - d0).num_days() as f64 / 365.25;
    if years < 1.0 || v0 <= 0.0 || v1 <= 0.0 {
        return None;
    }
    Some(((v1 / v0).powf(1.0 / years) - 1.0) * 100.0)
}

/// The base URL to use; `PENGUINFIN_FRED_URL` overrides it (for testing).
pub fn base_url() -> String {
    std::env::var("PENGUINFIN_FRED_URL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn parses_doc_example_and_skips_missing() {
        // Shape from the FRED docs' JSON example.
        let body = r#"{"realtime_start":"2013-08-14","realtime_end":"2013-08-14","observation_start":"1776-07-04",
            "observation_end":"9999-12-31","units":"lin","output_type":1,"file_type":"json","order_by":"observation_date",
            "sort_order":"asc","count":3,"offset":0,"limit":100000,"observations":[
            {"realtime_start":"2013-08-14","realtime_end":"2013-08-14","date":"1929-01-01","value":"1065.9"},
            {"realtime_start":"2013-08-14","realtime_end":"2013-08-14","date":"1930-01-01","value":"."},
            {"realtime_start":"2013-08-14","realtime_end":"2013-08-14","date":"1931-01-01","value":"912.1"}]}"#;
        let obs = parse_observations(body).unwrap();
        assert_eq!(obs, vec![(d(1929, 1, 1), 1065.9), (d(1931, 1, 1), 912.1)]);
    }

    #[test]
    fn parses_error_body() {
        let body = r#"{"error_code":400,"error_message":"Bad Request.  Variable api_key is not set.  Read https:\/\/fred.stlouisfed.org\/docs\/api\/api_key.html for more information."}"#;
        let err = parse_observations(body).unwrap_err();
        assert!(err.starts_with("FRED rejected the API key"), "{err}");
        assert!(!err.contains("  "));
    }

    #[test]
    fn api_key_format() {
        assert!(valid_api_key("abcdefghijklmnopqrstuvwxyz123456"));
        assert!(!valid_api_key("ABCDEFGHIJKLMNOPQRSTUVWXYZ123456"));
        assert!(!valid_api_key("short"));
    }

    #[test]
    fn cagr() {
        let r = annualized_return(d(2016, 1, 1), 100.0, d(2026, 1, 1), 200.0).unwrap();
        assert!((r - 7.177).abs() < 0.01, "{r}");
        assert!(annualized_return(d(2026, 1, 1), 100.0, d(2026, 3, 1), 110.0).is_none());
    }

    #[test]
    fn keys_roundtrip() {
        for i in Indicator::ALL {
            assert_eq!(Indicator::from_key(i.key()), Some(i));
        }
        for s in InflationSource::ALL {
            assert_eq!(InflationSource::from_key(s.key()), Some(s));
        }
    }
}
