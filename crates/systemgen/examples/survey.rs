//! `cargo run --release --example survey [N]`: statistics over N seeds, for tuning.

use systemgen::{generate, moons::ROCKY_WITHIN_RADII, BodyKind, Config, Tag, Zone};

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
    let mut kinds = std::collections::BTreeMap::new();
    let (mut with_megas, mut dimmed) = (0, 0);
    let (mut regular, mut captured, mut regular_large) = (0, 0, 0);
    let mut regular_per_giant = [0usize; 6];
    // Regular moon diameters, km: under 400, 400–800, 800–1,600, 1,600–3,200, 3,200 and up.
    let mut regular_sizes = [0usize; 5];
    // Inner regular moons by ice cover: under 0.1, 0.1–0.5, 0.5 and up.
    let (mut inner_ice, mut inner_outer_zone_ice) = ([0usize; 3], [0usize; 3]);
    let (mut inner_ice_worlds, mut volcanic, mut volcanic_giants) = (0, 0, 0);
    // Outer regular moons past the frost line: how many, how many Icy, ice cover 0.5 and up.
    let (mut outer_far, mut outer_far_icy, mut outer_far_iced) = (0, 0, 0);

    let start = std::time::Instant::now();
    for seed in 0..n {
        let system = generate(seed, &config);
        *classes
            .entry(format!("{:?}", system.star.class))
            .or_insert(0) += 1;
        with_megas += usize::from(!system.megastructures.is_empty());
        dimmed += usize::from(system.star.apparent_luminosity < system.star.luminosity);
        for m in &system.megastructures {
            *megas
                .entry(format!("{:?}/{:?}", m.kind, m.condition))
                .or_insert(0) += 1;
        }
        for b in &system.bodies {
            bodies += 1;
            *kinds.entry(format!("{:?}", b.kind)).or_insert(0) += 1;
            moons += b.moons.len();
            if b.kind.is_gaseous() {
                giants += 1;
                if b.orbit.distance < system.star.frost_line_au {
                    inner_giants += 1;
                }
                let family = b.moons.iter().filter(|m| m.regular_moon).count();
                volcanic_giants += usize::from(b.moons.iter().any(|m| m.volcanic));
                regular_per_giant[family.min(5)] += 1;
                regular += family;
                captured += b.moons.len() - family;
                for m in b.moons.iter().filter(|m| m.regular_moon) {
                    let diameter = 2.0 * m.radius_km;
                    regular_large += usize::from(diameter > 800.0);
                    let bucket = [400.0, 800.0, 1600.0, 3200.0]
                        .iter()
                        .take_while(|&&edge| diameter >= edge)
                        .count();
                    regular_sizes[bucket] += 1;
                    volcanic += usize::from(m.volcanic);
                    let ice = m.surface.map_or(0.0, |s| s.ice_cover);
                    let temperature = m.surface.map_or(0.0, |s| s.temperature_k);
                    let ice_bucket = usize::from(ice >= 0.1) + usize::from(ice >= 0.5);
                    let far = b.orbit.zone == Zone::Outer;
                    if m.orbit.distance <= ROCKY_WITHIN_RADII * b.radius_km {
                        inner_ice[ice_bucket] += 1;
                        if far {
                            inner_outer_zone_ice[ice_bucket] += 1;
                            inner_ice_worlds += usize::from(ice >= 0.5 && temperature <= 250.0);
                        }
                    } else if far {
                        outer_far += 1;
                        outer_far_icy += usize::from(m.kind == BodyKind::Icy);
                        outer_far_iced += usize::from(ice >= 0.5);
                    }
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
    println!("body kinds: {kinds:?}");
    println!(
        "settled mainworlds {:.1}%, outposts {:.2}/system, habitability > 0.5 {:.2}/system, gardens {:.3}/system, ruins {:.2}/system",
        100.0 * per(settled),
        per(outposts),
        per(habitable),
        per(gardens),
        per(ruins)
    );
    println!(
        "systems with megastructures {:.2}%, with a dimmed star {:.2}%",
        100.0 * per(with_megas),
        100.0 * per(dimmed)
    );
    println!("megastructures: {megas:?}");
    let per_giant = |x: usize| x as f64 / giants.max(1) as f64;
    println!(
        "per giant: {:.2} regular moons, {:.2} captured; giants with 0..=5 regular moons {regular_per_giant:?}",
        per_giant(regular),
        per_giant(captured)
    );
    println!(
        "regular moon diameters <400 / 400-800 / 800-1600 / 1600-3200 / 3200+ km: {regular_sizes:?}, {:.1}% above 800 km",
        100.0 * regular_large as f64 / regular.max(1) as f64
    );
    println!(
        "inner regular moons by ice cover <0.1 / 0.1-0.5 / 0.5+: {inner_ice:?}; past the frost line {inner_outer_zone_ice:?}, {inner_ice_worlds} of them iced over at <= 250 K"
    );
    println!(
        "volcanic moons: {volcanic}, {:.3}/giant, {:.2}/system, {:.1}% of inner regular moons; giants with one {:.1}%",
        per_giant(volcanic),
        per(volcanic),
        100.0 * volcanic as f64 / inner_ice.iter().sum::<usize>().max(1) as f64,
        100.0 * per_giant(volcanic_giants)
    );
    println!(
        "outer regular moons past the frost line: {outer_far}, {outer_far_icy} Icy, {outer_far_iced} with ice cover 0.5+"
    );
}
