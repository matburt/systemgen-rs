use systemgen::{
    generate, generate_with, Body, BodyKind, Config, Settlement, SplitMix64, StarChoice,
};

const SEEDS: std::ops::Range<u64> = 0..200;

#[test]
fn the_same_seed_gives_the_same_system() {
    let config = Config::default();
    for seed in SEEDS {
        assert_eq!(
            generate(seed, &config),
            generate(seed, &config),
            "seed {seed}"
        );
    }
}

#[test]
fn generate_is_generate_with_its_own_stream() {
    let config = Config::default();
    for seed in SEEDS.take(20) {
        let mut from_stream = generate_with(&mut SplitMix64::new(seed), &config);
        from_stream.seed = seed;
        assert_eq!(generate(seed, &config), from_stream);
    }
}

#[test]
fn different_seeds_give_different_systems() {
    let config = Config::default();
    let a = generate(1, &config);
    let b = generate(2, &config);
    assert_ne!(a, b);
}

#[test]
fn every_body_is_finite_and_bodies_run_outward() {
    let config = Config::default();
    for seed in SEEDS {
        let system = generate(seed, &config);
        assert!(!system.bodies.is_empty(), "seed {seed} has no bodies");
        for pair in system.bodies.windows(2) {
            assert!(
                pair[0].orbit.distance <= pair[1].orbit.distance,
                "seed {seed}"
            );
        }
        for (_, body) in system.all_bodies() {
            assert_finite(body, seed);
        }
        for planet in &system.bodies {
            for pair in planet.moons.windows(2) {
                assert!(
                    pair[0].orbit.distance <= pair[1].orbit.distance,
                    "seed {seed}"
                );
            }
        }
    }
}

fn assert_finite(b: &Body, seed: u64) {
    let mut values = vec![
        b.orbit.distance,
        b.orbit.eccentricity,
        b.mass_earth,
        b.radius_km,
    ];
    if let Some(s) = &b.surface {
        values.extend([s.gravity_g, s.pressure_bar, s.temperature_k, s.hydrosphere]);
    }
    assert!(
        values.iter().all(|v| v.is_finite() && *v >= 0.0),
        "seed {seed}: {b:?}"
    );
    assert!((0.0..=1.0).contains(&b.world.habitability), "seed {seed}");
}

#[test]
fn gas_giants_and_belts_have_no_surface() {
    let config = Config::default();
    for seed in SEEDS {
        for (_, b) in generate(seed, &config).all_bodies() {
            assert_eq!(
                b.surface.is_some(),
                b.kind.is_solid(),
                "seed {seed}: {:?}",
                b.kind
            );
        }
    }
}

#[test]
fn settlement_is_sparse_and_hangs_off_the_mainworld() {
    let config = Config::default();
    let mut settled_systems = 0;
    for seed in SEEDS {
        let system = generate(seed, &config);
        let main_settled = system
            .mainworld
            .is_some_and(|m| system.body(m).world.is_settled());
        settled_systems += usize::from(main_settled);
        for (r, b) in system.all_bodies() {
            if b.world.is_settled() && Some(r) != system.mainworld {
                assert!(
                    main_settled,
                    "seed {seed}: outpost without a settled mainworld"
                );
                assert!(b.world.population <= 3, "seed {seed}");
            }
        }
    }
    // Few systems are settled, but not none.
    let share = settled_systems as f64 / SEEDS.count() as f64;
    assert!((0.03..0.4).contains(&share), "{share}");
}

#[test]
fn no_one_lives_anywhere_when_settlement_is_off() {
    let config = Config {
        settlement: Settlement {
            settled_chance: 0.0,
            ..Settlement::default()
        },
        ..Config::default()
    };
    for seed in SEEDS.take(50) {
        let system = generate(seed, &config);
        assert!(
            system.all_bodies().all(|(_, b)| !b.world.is_settled()),
            "seed {seed}"
        );
    }
}

#[test]
fn sunlike_stars_usually_have_a_gas_giant_past_the_frost_line() {
    let config = Config {
        star: StarChoice::Mass(1.0),
        ..Config::default()
    };
    let with_giant = SEEDS
        .take(50)
        .filter(|&seed| {
            let system = generate(seed, &config);
            system.bodies.iter().any(|b| {
                b.kind == BodyKind::GasGiant && b.orbit.distance > system.star.frost_line_au
            })
        })
        .count();
    assert!(with_giant >= 40, "{with_giant}/50");
}
