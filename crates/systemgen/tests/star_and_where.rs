use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;

use systemgen::{
    generate, generate_star, generate_star_with, generate_where, generate_with, Config,
    Megastructures, SpectralClass, SplitMix64, StarChoice, StarSystem, StarWeights, Zone,
};

fn star_choices() -> [StarChoice; 6] {
    [
        StarChoice::default(),
        StarChoice::Weighted(StarWeights::REALISTIC),
        StarChoice::Class(SpectralClass::G),
        StarChoice::Class(SpectralClass::K),
        StarChoice::Class(SpectralClass::M),
        StarChoice::Mass(1.3),
    ]
}

#[test]
fn generate_star_is_the_full_generations_star_dimming_included() {
    for choice in star_choices() {
        // Forced megastructures dim most stars and strip or ring their planets.
        for (megastructures, seeds) in [
            (Megastructures::default(), 300),
            (forced_megastructures(), 300),
        ] {
            let config = Config {
                star: choice,
                megastructures,
                ..Config::default()
            };
            for seed in 0..seeds {
                let full = generate(seed, &config).star;
                assert_eq!(
                    generate_star(seed, &config),
                    full,
                    "seed {seed}, {choice:?}"
                );
            }
        }
    }
}

#[test]
fn some_forced_megastructures_dim_the_star_alone() {
    let config = Config {
        megastructures: forced_megastructures(),
        ..Config::default()
    };
    let dimmed = (0..100)
        .filter(|&seed| {
            let star = generate_star(seed, &config);
            star.apparent_luminosity < star.luminosity
        })
        .count();
    assert!(dimmed > 0);
}

#[test]
fn generate_star_with_draws_like_generate_with() {
    let config = Config {
        megastructures: forced_megastructures(),
        ..Config::default()
    };
    for seed in 0..50 {
        let (mut a, mut b) = (SplitMix64::new(seed), SplitMix64::new(seed));
        let full = generate_with(&mut a, &config).star;
        let alone = generate_star_with(&mut b, &config);
        assert_eq!(alone, full, "seed {seed}");
        assert_eq!(
            systemgen::Rng::next_u64(&mut a),
            systemgen::Rng::next_u64(&mut b)
        );
    }
}

fn forced_megastructures() -> Megastructures {
    Megastructures {
        chance: 1.0,
        ..Megastructures::default()
    }
}

/// The best habitability of a solid body in the habitable zone, or 0.
fn habitable_zone_score(system: &StarSystem) -> f64 {
    system
        .all_bodies()
        .filter(|(_, b)| b.kind.is_solid() && b.orbit.zone == Zone::Habitable)
        .map(|(_, b)| b.world.habitability)
        .fold(0.0, f64::max)
}

#[test]
fn generate_where_is_deterministic() {
    let config = Config::default();
    for seed in 0..20 {
        let a = generate_where(seed, &config, 16, habitable_zone_score, 0.3);
        let b = generate_where(seed, &config, 16, habitable_zone_score, 0.3);
        assert_eq!(a, b, "seed {seed}");
    }
}

#[test]
fn generate_where_returns_a_system_its_seed_regenerates() {
    let config = Config::default();
    for seed in 0..20 {
        let found = generate_where(seed, &config, 16, habitable_zone_score, 0.3);
        assert_eq!(found.system.seed, found.sub_seed);
        assert_eq!(
            found.system,
            generate(found.sub_seed, &config),
            "seed {seed}"
        );
    }
}

#[test]
fn generate_where_tries_the_seed_itself_first() {
    let config = Config::default();
    for seed in 0..20 {
        let found = generate_where(seed, &config, 8, |_| 1.0, 0.0);
        assert_eq!((found.sub_seed, found.attempt), (seed, 0));
        assert_eq!(found.system, generate(seed, &config));
    }
}

#[test]
fn generate_where_keeps_the_best_try_when_none_is_good_enough() {
    let config = Config::default();
    for seed in 0..10 {
        let scores = RefCell::new(Vec::new());
        let recorded = |system: &StarSystem| {
            let score = habitable_zone_score(system);
            scores.borrow_mut().push(score);
            score
        };
        let best = generate_where(seed, &config, 8, recorded, f64::INFINITY);
        let scores = scores.into_inner();
        assert_eq!(scores.len(), 8);
        let top = scores.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        assert_eq!(best.score, top, "seed {seed}");
        assert_eq!(habitable_zone_score(&best.system), top, "seed {seed}");
        let first_top = scores.iter().position(|&s| s == top).unwrap();
        assert_eq!(best.attempt as usize, first_top, "seed {seed}");
    }
}

#[test]
fn generate_where_reports_the_try_that_reached_good_enough() {
    let config = Config::default();
    for seed in 0..20 {
        let used = Cell::new(0);
        let counted = |system: &StarSystem| {
            used.set(used.get() + 1);
            habitable_zone_score(system)
        };
        let found = generate_where(seed, &config, 64, counted, 0.3);
        if found.score >= 0.3 {
            assert_eq!(found.attempt + 1, used.get(), "seed {seed}");
        }
    }
}

#[test]
fn generate_where_never_repeats_a_sub_seed_across_neighbouring_seeds() {
    let config = Config::default();
    let tried = RefCell::new(BTreeSet::new());
    let record = |system: &StarSystem| {
        assert!(tried.borrow_mut().insert(system.seed), "{}", system.seed);
        0.0
    };
    for seed in 0..20 {
        generate_where(seed, &config, 8, record, 1.0);
    }
    assert_eq!(tried.borrow().len(), 20 * 8);
}

/// How many tries each of 100 seeds needed to find a habitable-zone world of habitability
/// 0.3 or more, `None` where `tries` were not enough.
fn tries_to_a_habitable_zone_world(class: SpectralClass, tries: u32) -> Vec<Option<u32>> {
    let config = Config {
        star: StarChoice::Class(class),
        ..Config::default()
    };
    (0..100)
        .map(|seed| {
            let used = Cell::new(0);
            let counted = |system: &StarSystem| {
                used.set(used.get() + 1);
                habitable_zone_score(system)
            };
            let found = generate_where(seed, &config, tries, counted, 0.3);
            (found.score >= 0.3).then(|| used.get())
        })
        .collect()
}

fn found_within(needed: &[Option<u32>], tries: u32) -> usize {
    needed
        .iter()
        .filter(|n| n.is_some_and(|n| n <= tries))
        .count()
}

#[test]
fn g_stars_find_a_habitable_zone_world_within_64_tries() {
    let needed = tries_to_a_habitable_zone_world(SpectralClass::G, 64);
    // Measured: 100 of 100 (about 8.5% of G systems qualify on a single try).
    assert!(found_within(&needed, 64) >= 95, "{needed:?}");
}

#[test]
#[ignore = "a measurement, not a guard: homes are G stars; run with --ignored when tuning K worlds"]
fn k_stars_find_a_habitable_zone_world_within_256_tries() {
    let needed = tries_to_a_habitable_zone_world(SpectralClass::K, 256);
    // Measured: 61 of 100 within 64 tries and 100 of 100 within 256 (about 1.7% of K systems
    // qualify on a single try: their habitable-zone worlds are mostly too cold and too thin).
    assert!(found_within(&needed, 64) >= 50, "{needed:?}");
    assert!(found_within(&needed, 256) >= 95, "{needed:?}");
}
