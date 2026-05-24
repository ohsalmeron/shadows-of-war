fn main() {
    // Let's test varying tile counts to see what produces 1.4K with scale 1.0
    // and what produced 47K with scale 12.0.

    let target1 = 1400.0;

    for t in 0..2000000 {
        let t_f64 = t as f64;
        let bonus = t_f64.powf(0.625);
        let cap_scale_1 = 100.0 + bonus * 1.0;
        if (cap_scale_1 - target1).abs() < 1.0 {
            println!("Tiles needed for 1.4K cap (scale 1.0): {}", t);
            break;
        }
    }

    let target2 = 47000.0;
    for t in 0..2000000 {
        let t_f64 = t as f64;
        let bonus = t_f64.powf(0.625);
        let cap_scale_12 = 100.0 + bonus * 12.0;
        if (cap_scale_12 - target2).abs() < 10.0 {
            println!("Tiles needed for 47K cap (scale 12.0): {}", t);
            break;
        }
    }
}
