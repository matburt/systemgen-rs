//! `cargo run --release --example survey [N]`: statistics over N seeds, for tuning.

use systemgen::{generate, BodyKind, Config, Tag};

fn main() {
    let n: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(2000);
    let config = Config::default();

    let (mut bodies, mut moons, mut giants, mut inner_giants, mut belts) = (0, 0, 0, 0, 0);
    let (mut settled, mut outposts, mut gardens, mut ruins, mut habitable) = (0, 0, 0, 0, 0);
    let mut classes = std::collections::BTreeMap::new();
    let mut megas = std::collections::BTreeMap::new();

    let start = std::time::Instant::now();
    for seed in 0..n {
        let system = generate(seed, &config);
        *classes
            .entry(format!("{:?}", system.star.class))
            .or_insert(0) += 1;
        for m in &system.megastructures {
            *megas
                .entry(format!("{:?}/{:?}", m.kind, m.condition))
                .or_insert(0) += 1;
        }
        for b in &system.bodies {
            bodies += 1;
            moons += b.moons.len();
            if b.kind.is_gaseous() {
                giants += 1;
                if b.orbit.distance < system.star.frost_line_au {
                    inner_giants += 1;
                }
            }
            if b.kind == BodyKind::Belt {
                belts += 1;
            }
        }
        for (r, b) in system.all_bodies() {
            let w = &b.world;
            if w.habitability > 0.5 {
                habitable += 1;
            }
            if w.tags.contains(&Tag::Garden) {
                gardens += 1;
            }
            if w.remnant.is_some() {
                ruins += 1;
            }
            if w.is_settled() {
                if Some(r) == system.mainworld {
                    settled += 1;
                } else {
                    outposts += 1;
                }
            }
        }
    }
    let per = |x: usize| x as f64 / n as f64;
    println!(
        "{n} systems in {:.2?} ({:.2?} each)",
        start.elapsed(),
        start.elapsed() / n as u32
    );
    println!("stars: {classes:?}");
    println!(
        "per system: {:.1} bodies, {:.1} moons, {:.2} gaseous ({:.2} inside the frost line), {:.2} belts",
        per(bodies),
        per(moons),
        per(giants),
        per(inner_giants),
        per(belts)
    );
    println!(
        "settled mainworlds {:.1}%, outposts {:.2}/system, habitability > 0.5 {:.2}/system, gardens {:.3}/system, ruins {:.2}/system",
        100.0 * per(settled),
        per(outposts),
        per(habitable),
        per(gardens),
        per(ruins)
    );
    println!("megastructures: {megas:?}");
}
