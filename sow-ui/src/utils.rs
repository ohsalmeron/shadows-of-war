pub fn format_number(mut num: f64) -> String {
    num = num.max(0.0);
    if num >= 10_000_000.0 {
        let value = (num / 100_000.0).floor() / 10.0;
        format!("{:.1}M", value)
    } else if num >= 1_000_000.0 {
        let value = (num / 10_000.0).floor() / 100.0;
        format!("{:.2}M", value)
    } else if num >= 100_000.0 {
        format!("{}K", (num / 1000.0).floor())
    } else if num >= 10_000.0 {
        let value = (num / 100.0).floor() / 10.0;
        format!("{:.1}K", value)
    } else if num >= 1_000.0 {
        let value = (num / 10.0).floor() / 100.0;
        format!("{:.2}K", value)
    } else {
        format!("{:.0}", num.floor())
    }
}
