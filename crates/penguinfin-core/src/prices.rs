//! Price lookup for holdings.
//!
//! V1 is fully offline: prices are typed in by hand. A future online quote source
//! only needs to implement [`PriceProvider`]; the UI calls `refresh_prices`
//! and stores the result through the normal `Db::save_holding` path.

use crate::model::Holding;
use chrono::NaiveDate;

#[derive(Debug, Clone, PartialEq)]
pub struct Quote {
    pub symbol: String,
    pub price: f64,
    pub as_of: NaiveDate,
}

pub trait PriceProvider {
    fn name(&self) -> &'static str;
    /// Fetch latest prices for the given symbols. Missing symbols are simply left out.
    fn quotes(&self, symbols: &[String]) -> Result<Vec<Quote>, String>;
}

/// The default provider: no network, no quotes.
pub struct ManualPrices;

impl PriceProvider for ManualPrices {
    fn name(&self) -> &'static str {
        "Manual entry"
    }
    fn quotes(&self, _symbols: &[String]) -> Result<Vec<Quote>, String> {
        Ok(Vec::new())
    }
}

/// Apply quotes to holdings, returning the holdings that changed.
pub fn refresh_prices(provider: &dyn PriceProvider, holdings: &[Holding]) -> Result<Vec<Holding>, String> {
    let mut symbols: Vec<String> = holdings.iter().map(|h| h.symbol.to_uppercase()).collect();
    symbols.sort();
    symbols.dedup();
    let quotes = provider.quotes(&symbols)?;
    Ok(holdings
        .iter()
        .filter_map(|h| {
            let q = quotes.iter().find(|q| q.symbol.eq_ignore_ascii_case(&h.symbol))?;
            Some(Holding { price: q.price, price_updated: Some(q.as_of), ..h.clone() })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed;
    impl PriceProvider for Fixed {
        fn name(&self) -> &'static str {
            "fixed"
        }
        fn quotes(&self, symbols: &[String]) -> Result<Vec<Quote>, String> {
            Ok(symbols
                .iter()
                .filter(|s| s.as_str() == "VTI")
                .map(|s| Quote { symbol: s.clone(), price: 300.0, as_of: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap() })
                .collect())
        }
    }

    #[test]
    fn refresh_updates_only_quoted() {
        let hs = vec![
            Holding { id: 1, account_id: 1, symbol: "vti".into(), shares: 1.0, price: 1.0, price_updated: None },
            Holding { id: 2, account_id: 1, symbol: "XYZ".into(), shares: 1.0, price: 1.0, price_updated: None },
        ];
        let out = refresh_prices(&Fixed, &hs).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].price, 300.0);
        assert!(refresh_prices(&ManualPrices, &hs).unwrap().is_empty());
    }
}
