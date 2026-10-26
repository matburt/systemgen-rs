//! Regular moons: the families gas and ice giants build in their circumplanetary disks, which
//! Accrete (capture only) does not model. Runs on the accrete layer's planets after their
//! environment pass, drawing from the accrete stream.
//!
//! Each giant gets 2–5 moons holding about 10⁻⁴ of its mass between them (Canup & Ward 2006),
//! scattered by up to a factor of two either way and split unevenly so one or two dominate.
//! Their orbits are prograde, nearly circular and evenly spaced in log distance, from 3–6
//! planet radii out to the lesser of 15–30 radii and 0.4 of the Hill sphere, where the disk is
//! truncated. A cramped giant, close to its star, gets fewer moons or none. A moon whose
//! periapsis falls within twice the Roche limit becomes a ring, as a captured one does.
//!
//! Moons inside [`ROCKY_WITHIN_RADII`] planet radii are rocky; farther out they are icy when
//! the planet is past the frost line and rocky inside it. A rocky innermost moon with a
//! neighbour to keep its orbit eccentric is tidally heated and may be volcanic (Io), which
//! shows as `tectonic`. Environment is derived as for captured moons: temperature from the
//! star's distance, gas retention from mass and radius. Captured moons stay, as the irregular
//! moons.

use crate::accrete::consts::KM_PER_AU;
use crate::accrete::{roche_limit_au, Planetesimal, Ring, System};
use crate::rng::Rng;

/// The satellite-system to planet mass ratio.
const MASS_RATIO: f64 = 1.0e-4;
/// The ratio is scattered by 10^±this.
const MASS_SCATTER_DEX: f64 = 0.3;
const MIN_MOONS: u32 = 2;
const MAX_MOONS: u32 = 5;
/// Inner edge of the family, in planet radii (Io 5.9, Mimas 3.1, Miranda 5.1).
const INNER_RADII: (f64, f64) = (3.0, 6.0);
/// Outer edge, in planet radii (Callisto 26, Titan 20, Oberon 23).
const OUTER_RADII: (f64, f64) = (15.0, 30.0);
/// The circumplanetary disk is truncated at this fraction of the Hill sphere.
const DISK_HILL_FRACTION: f64 = 0.4;
/// Neighbouring moons sit at least this ratio of distances apart.
const MIN_SPACING: f64 = 1.35;
/// Each moon's distance is jittered by up to this fraction of its log-even place.
const JITTER: f64 = 0.04;
const MAX_ECCENTRICITY: f64 = 0.01;
/// A moon's share of the family's mass is proportional to a uniform draw from this range,
/// squared.
const SHARE_RANGE: (f64, f64) = (0.15, 1.0);
/// Moons this close to their planet, in planet radii, form rocky (Io 5.9, Europa 9.4).
pub const ROCKY_WITHIN_RADII: f64 = 8.0;
const VOLCANIC_CHANCE: f64 = 0.5;
const ROCKY_ZONE: i32 = 1;
const ICY_ZONE: i32 = 3;

/// Gives each gaseous planet of `system` its regular moons. `frost_line_au` decides which
/// giants' outer moons are icy.
pub(crate) fn form(system: &mut System, frost_line_au: f64, rng: &mut dyn Rng) {
    let star = &system.primary_star;
    let env = Env {
        luminosity: star.stellar_luminosity,
        main_seq_age: star.main_seq_age,
        ecosphere: star.ecosphere,
        frost_line_au,
    };
    for planet in system.planets.iter_mut().filter(|p| p.is_gas_giant) {
        form_family(planet, &env, rng);
    }
}

struct Env {
    luminosity: f64,
    main_seq_age: f64,
    ecosphere: (f64, f64),
    frost_line_au: f64,
}

fn form_family(planet: &mut Planetesimal, env: &Env, rng: &mut dyn Rng) {
    let radius_au = planet.radius / KM_PER_AU;
    let inner = rng.range_f64(INNER_RADII.0, INNER_RADII.1) * radius_au;
    let outer = (rng.range_f64(OUTER_RADII.0, OUTER_RADII.1) * radius_au)
        .min(DISK_HILL_FRACTION * planet.hill_sphere);
    let wanted = MIN_MOONS + rng.below(MAX_MOONS - MIN_MOONS + 1);
    let count = wanted.min(room_for(inner, outer));
    if count == 0 {
        return;
    }
    let total = planet.mass * MASS_RATIO * 10f64.powf(rng.range_f64(-1.0, 1.0) * MASS_SCATTER_DEX);
    let shares: Vec<f64> = (0..count)
        .map(|_| rng.range_f64(SHARE_RANGE.0, SHARE_RANGE.1).powi(2))
        .collect();
    let share_sum: f64 = shares.iter().sum();
    let past_frost_line = planet.a >= env.frost_line_au;

    for (i, share) in shares.iter().enumerate() {
        let place = if count == 1 {
            0.0
        } else {
            i as f64 / f64::from(count - 1)
        };
        let jitter = 1.0 + rng.range_f64(-JITTER, JITTER);
        let a = inner * (outer / inner).powf(place) * jitter;
        let e = rng.range_f64(0.0, MAX_ECCENTRICITY);
        let icy = past_frost_line && a > ROCKY_WITHIN_RADII * radius_au;
        let zone = if icy { ICY_ZONE } else { ROCKY_ZONE };
        let mut moon = Planetesimal::regular_moon(a, e, total * share / share_sum, zone);
        moon.distance_to_primary_star = planet.a;

        let roche = roche_limit_au(&planet.mass, &moon.mass, &moon.radius);
        if a * (1.0 - e) <= 2.0 * roche {
            planet.rings.push(Ring::from_planet(roche, &moon));
            continue;
        }
        moon.derive_planetary_environment(
            &env.luminosity,
            &planet.mass,
            &env.main_seq_age,
            &env.ecosphere,
            rng,
        );
        if i == 0 && !icy && count > 1 && rng.chance(VOLCANIC_CHANCE) {
            moon.tectonic_activity = true;
        }
        planet.moons.push(moon);
    }
}

/// How many moons fit between `inner` and `outer` at [`MIN_SPACING`], jitter allowed for.
fn room_for(inner: f64, outer: f64) -> u32 {
    if !(inner > 0.0 && outer > inner) {
        return 0;
    }
    let spacing = MIN_SPACING * (1.0 + JITTER) / (1.0 - JITTER);
    1 + ((outer / inner).ln() / spacing.ln()).floor() as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accrete::{planetary_system, Disk};
    use crate::rng::SplitMix64;
    use crate::star::Star;

    const JUPITER_SOLAR_MASSES: f64 = 9.55e-4;

    fn jupiter_system(seed: u64) -> System {
        let mut system = System::set_initial_conditions(1.0, 0.0015, 50.0, 0.2, 1.2e-5);
        let star = &system.primary_star;
        let jupiter = Planetesimal::random_planet(
            star.stellar_luminosity,
            star.stellar_mass,
            5.2,
            0.05,
            JUPITER_SOLAR_MASSES,
            0,
            &mut SplitMix64::new(seed),
        );
        system.planets.push(jupiter);
        let frost_line = Star::from_primary(&system.primary_star).frost_line_au;
        form(&mut system, frost_line, &mut SplitMix64::new(seed));
        system
    }

    fn regular(p: &Planetesimal) -> impl Iterator<Item = &Planetesimal> {
        p.moons.iter().filter(|m| m.is_regular_moon)
    }

    #[test]
    fn a_jupiter_at_5_au_gets_2_to_5_moons_of_about_a_ten_thousandth_its_mass() {
        for seed in 0..100 {
            let system = jupiter_system(seed);
            let jupiter = &system.planets[0];
            assert!(jupiter.is_gas_giant);
            let moons: Vec<_> = regular(jupiter).collect();
            assert!(
                (2..=5).contains(&moons.len()),
                "seed {seed}: {}",
                moons.len()
            );
            let ratio = moons.iter().map(|m| m.mass).sum::<f64>() / jupiter.mass;
            assert!((0.4e-4..2.1e-4).contains(&ratio), "seed {seed}: {ratio}");
        }
    }

    #[test]
    fn regular_moons_sit_outside_the_roche_limit_and_inside_the_hill_sphere() {
        let mut checked = 0;
        for seed in 0..300 {
            let mut rng = SplitMix64::new(seed);
            let mass = rng.range_f64(0.3, 1.5);
            let mut system = planetary_system(mass, &Disk::default(), &mut rng);
            let frost_line = Star::from_primary(&system.primary_star).frost_line_au;
            form(&mut system, frost_line, &mut rng);
            for planet in &system.planets {
                for m in regular(planet) {
                    let roche = roche_limit_au(&planet.mass, &m.mass, &m.radius);
                    assert!(m.a * (1.0 - m.e) > 2.0 * roche, "seed {seed}");
                    assert!(m.a * (1.0 + m.e) < planet.hill_sphere, "seed {seed}");
                    assert!(m.radius.is_finite() && m.radius > 0.0, "seed {seed}");
                    checked += 1;
                }
            }
        }
        assert!(checked > 100, "{checked}");
    }

    #[test]
    fn the_same_seed_forms_the_same_family() {
        for seed in 0..20 {
            assert_eq!(jupiter_system(seed), jupiter_system(seed));
        }
    }
}
