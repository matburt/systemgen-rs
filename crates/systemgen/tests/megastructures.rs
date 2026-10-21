use systemgen::{
    generate, BodyKind, Condition, Config, Host, MegaKind, Megastructures, SpectralClass,
};

const SEEDS: std::ops::Range<u64> = 0..400;

fn always() -> Config {
    Config {
        megastructures: Megastructures {
            chance: 1.0,
            ..Megastructures::default()
        },
        ..Config::default()
    }
}

fn never() -> Config {
    Config {
        megastructures: Megastructures {
            chance: 0.0,
            ..Megastructures::default()
        },
        ..Config::default()
    }
}

#[test]
fn a_system_without_megastructures_is_unchanged_by_the_layer() {
    let (default, never) = (Config::default(), never());
    for seed in SEEDS {
        let with = generate(seed, &default);
        if with.megastructures.is_empty() {
            assert_eq!(with, generate(seed, &never), "seed {seed}");
        }
    }
}

#[test]
fn megastructures_are_rare_by_default() {
    let config = Config::default();
    let with = SEEDS
        .filter(|&s| !generate(s, &config).megastructures.is_empty())
        .count();
    let share = with as f64 / SEEDS.count() as f64;
    assert!((0.005..0.12).contains(&share), "{share}");
}

#[test]
fn every_structure_fits_its_star_and_host() {
    let config = always();
    for seed in SEEDS {
        let system = generate(seed, &config);
        assert!(!system.megastructures.is_empty(), "seed {seed}");
        for m in &system.megastructures {
            assert!(m.radius.is_finite() && m.radius > 0.0, "seed {seed}: {m:?}");
            assert!((0.0..=1.0).contains(&m.coverage), "seed {seed}: {m:?}");
            assert_eq!(
                m.era.is_none(),
                m.condition == Condition::Active,
                "seed {seed}"
            );
            match m.kind {
                MegaKind::Ringworld => assert!(
                    matches!(
                        system.star.class,
                        SpectralClass::K | SpectralClass::G | SpectralClass::F
                    ),
                    "seed {seed}: ringworld round a {:?}",
                    system.star.class
                ),
                MegaKind::OrbitalRing => {
                    let Host::Body(r) = m.host else {
                        panic!("seed {seed}: orbital ring round the star");
                    };
                    let planet = system.body(r);
                    assert!(planet.kind.is_solid(), "seed {seed}");
                    assert!(m.radius > planet.radius_km, "seed {seed}");
                }
                _ => assert_eq!(m.host, Host::Star, "seed {seed}"),
            }
        }
    }
}

#[test]
fn a_shell_dims_the_star_and_strips_its_inner_system() {
    let config = always();
    let mut shells = 0;
    for seed in SEEDS {
        let system = generate(seed, &config);
        for m in &system.megastructures {
            if !matches!(m.kind, MegaKind::DysonSphere | MegaKind::MatrioshkaBrain) {
                continue;
            }
            shells += 1;
            assert!(
                system.star.apparent_luminosity < system.star.luminosity,
                "seed {seed}"
            );
            let inner_solid = system.bodies.iter().any(|b| {
                matches!(b.kind, BodyKind::Rocky | BodyKind::Icy | BodyKind::Dwarf)
                    && b.orbit.distance <= m.radius * 3.0
            });
            assert!(
                !inner_solid,
                "seed {seed}: solid body inside the shell's reach"
            );
        }
    }
    assert!(shells > 0, "no shells in {} seeds", SEEDS.count());
}

#[test]
fn living_structures_need_a_high_tech_settled_mainworld() {
    let config = always();
    for seed in SEEDS {
        let system = generate(seed, &config);
        if system
            .megastructures
            .iter()
            .any(|m| m.condition == Condition::Active)
        {
            let main = system.body(system.mainworld.expect("a mainworld"));
            assert!(
                main.world.is_settled() && main.world.tech >= 12,
                "seed {seed}"
            );
        }
    }
}
