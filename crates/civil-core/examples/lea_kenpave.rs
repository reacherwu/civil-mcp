//! Compare civil-core LEA with the published KENPAVE table (data/validation/kenpave_3layer.csv,
//! from PyMastic, Apache-2.0). Prints max relative errors for z = 1..29 in.
use civil_core::lea::{response, LeaLayer};
fn main() {
    let ls = vec![LeaLayer { h: 6.0, e: 500e3, nu: 0.35 }, LeaLayer { h: 10.0, e: 40e3, nu: 0.4 }, LeaLayer { h: 0.0, e: 10e3, nu: 0.45 }];
    let t = std::fs::read_to_string(std::env::args().nth(1).unwrap_or("data/validation/kenpave_3layer.csv".into())).unwrap();
    let (mut ew, mut es, mut ee, mut ez) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    println!("z_in,w_ours,w_ken,sz_ours,sz_ken,er_ours,er_ken,ez_ours,ez_ken");
    for l in t.lines().skip(2) {
        let v: Vec<f64> = l.split(',').map(|x| x.parse().unwrap()).collect();
        let o = response(&ls, 100.0, 5.99, 0.0, v[0]);
        println!("{},{:.5},{},{:.3},{},{:.4e},{},{:.4e},{}", v[0], o.w, v[1], -o.sz, v[2], -o.er, v[6], -o.ez, v[5]);
        ew = ew.max((o.w - v[1]).abs() / v[1]);
        es = es.max((-o.sz - v[2]).abs() / v[2]);
        ee = ee.max((-o.er - v[6]).abs() / v[6].abs());
        ez = ez.max((-o.ez - v[5]).abs() / v[5].abs());
    }
    println!("# max rel diff z=1..29 in: deflection {:.3}%, sigma_z {:.3}%, eps_r {:.3}%, eps_z {:.3}%", ew * 100.0, es * 100.0, ee * 100.0, ez * 100.0);
}
