//! Money and number formatting shared by every PenguinFin interface.


fn group_thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// "$1,234" (rounded to whole units).
pub fn money(v: f64, sym: &str) -> String {
    let neg = v < -0.5;
    let n = v.abs().round() as u64;
    format!("{}{sym}{}", if neg { "-" } else { "" }, group_thousands(n))
}

/// "$1,234.56"
pub fn money_cents(v: f64, sym: &str) -> String {
    let neg = v < -0.005;
    let cents = (v.abs() * 100.0).round() as u64;
    format!("{}{sym}{}.{:02}", if neg { "-" } else { "" }, group_thousands(cents / 100), cents % 100)
}

/// Short axis labels: "$1.2M", "$350k".
pub fn money_short(v: f64, sym: &str) -> String {
    let a = v.abs();
    let sign = if v < 0.0 { "-" } else { "" };
    if a >= 1e6 {
        format!("{sign}{sym}{:.1}M", a / 1e6)
    } else if a >= 1e3 {
        format!("{sign}{sym}{:.0}k", a / 1e3)
    } else {
        format!("{sign}{sym}{a:.0}")
    }
}

pub fn parse_num(s: &str) -> Option<f64> {
    let cleaned: String = s.chars().filter(|c| !matches!(c, ',' | '$' | ' ' | '%')).collect();
    if cleaned.is_empty() {
        return None;
    }
    cleaned.parse::<f64>().ok().filter(|v| v.is_finite())
}

pub fn num_str(v: f64) -> String {
    if v == v.trunc() { format!("{v:.0}") } else { format!("{v}") }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formatting() {
        assert_eq!(money(1234567.4, "$"), "$1,234,567");
        assert_eq!(money(-950.0, "$"), "-$950");
        assert_eq!(money(0.2, "$"), "$0");
        assert_eq!(money_cents(1234.5, "€"), "€1,234.50");
        assert_eq!(money_short(1_250_000.0, "$"), "$1.2M");
        assert_eq!(money_short(-35_000.0, "$"), "-$35k");
        assert_eq!(parse_num("$1,200.50"), Some(1200.5));
        assert_eq!(parse_num(""), None);
        assert_eq!(parse_num("abc"), None);
    }
}
