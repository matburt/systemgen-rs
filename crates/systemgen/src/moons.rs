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
//! Moons inside [`ROCKY_WITHIN_RADII`] planet radii are *inner*: they formed inside the
//! giant's own snow line, where its young glow kept the disk too warm for ice (Io), so they
//! are rocky and dry: the environment pass gives them no water, and so no ice, wherever their
//! giant orbits. Farther out moons are icy when the planet is past the frost line and rocky
//! inside it, and keep the environment pass's water.
//!
//! An inner moon with an outer neighbour is held in an eccentric orbit by resonance, as Io is
//! by Europa, and is tidally heated. Its heat flow scales from Io's (2.5 W/m² at 5.9 Jupiter
//! radii, 421,700 km) as `M_planet^2.5 · R_moon^3 · a^-7.5`, the tidal dissipation per unit
//! surface at Io's eccentricity and rigidity: it grows with the planet's mass and the moon's
//! size and falls steeply with distance in planet radii (half as far, 180 times the heat). It
//! is capped at [`MAX_TIDAL_HEATING_W_M2`], past which a molten interior stops dissipating
//! more, and adds to the surface energy balance, so `T⁴` rises by `flux / σ`: an Io at 5 AU
//! warms by about 6 K, a capped moon there by about 40 K. A moon heated by at least
//! [`VOLCANIC_MIN_W_M2`] is `Body::volcanic`; `tectonic` stays Accrete's broad flag. Outer
//! moons and moons without a neighbour are not heated. (Enceladus, at Saturn, comes out at
//! about 0.02 W/m², near its measured flow, and is not volcanic.)
//!
//! Environment is otherwise derived as for captured moons: temperature from the star's
//! distance, gas retention from mass and radius. Captured moons stay, as the irregular moons.

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
/// Moons this close to their planet, in planet radii, form rocky and dry (Io 5.9, Europa 9.4).
pub const ROCKY_WITHIN_RADII: f64 = 8.0;
/// Io's tidal heat flow, W/m² (Veeder et al. 2012: about 2–3).
const IO_HEATING_W_M2: f64 = 2.5;
const IO_DISTANCE_KM: f64 = 421_700.0;
const IO_RADIUS_KM: f64 = 1821.6;
const JUPITER_SOLAR_MASSES: f64 = 9.546e-4;
/// Tidal heating stops growing here, W/m²: ten Ios.
pub const MAX_TIDAL_HEATING_W_M2: f64 = 25.0;
/// Tidal heat flow at which a moon reads as volcanic, W/m²: the low end of Io's 2–3, over
/// twenty times Earth's internal heat flow (0.09).
pub const VOLCANIC_MIN_W_M2: f64 = 2.0;
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
        let is_inner = a <= ROCKY_WITHIN_RADII * radius_au;
        let icy = past_frost_line && !is_inner;
        let zone = if icy { ICY_ZONE } else { ROCKY_ZONE };
        let mut moon = Planetesimal::regular_moon(a, e, total * share / share_sum, zone);
        moon.distance_to_primary_star = planet.a;
        moon.is_dry = is_inner;
        let has_outer_neighbour = i + 1 < shares.len();
        if is_inner && has_outer_neighbour {
            moon.tidal_heating_w_m2 = tidal_heating_w_m2(planet.mass, moon.radius, a);
            moon.is_volcanic = moon.tidal_heating_w_m2 >= VOLCANIC_MIN_W_M2;
        }

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
        planet.moons.push(moon);
    }
}

/// The tidal heat flow, W/m², of a moon of `radius_km` at `a_au` from a planet of
/// `planet_mass` solar masses, held at Io's eccentricity: Io's, scaled as
/// `M^2.5 · R^3 · a^-7.5` and capped at [`MAX_TIDAL_HEATING_W_M2`].
fn tidal_heating_w_m2(planet_mass: f64, radius_km: f64, a_au: f64) -> f64 {
    let flux = IO_HEATING_W_M2
        * (planet_mass / JUPITER_SOLAR_MASSES).powf(2.5)
        * (radius_km / IO_RADIUS_KM).powi(3)
        * (IO_DISTANCE_KM / (a_au * KM_PER_AU)).powf(7.5);
    flux.min(MAX_TIDAL_HEATING_W_M2)
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

    fn is_inner(moon: &Planetesimal, planet: &Planetesimal) -> bool {
        moon.a <= ROCKY_WITHIN_RADII * planet.radius / KM_PER_AU
    }

    #[test]
    fn inner_moons_of_a_jupiter_at_5_au_are_dry_rock_not_ice() {
        let mut inner = 0;
        for seed in 0..200 {
            let system = jupiter_system(seed);
            let jupiter = &system.planets[0];
            for m in regular(jupiter).filter(|m| is_inner(m, jupiter)) {
                assert_eq!(m.orbit_zone, ROCKY_ZONE, "seed {seed}");
                assert!(m.ice_cover < 0.5, "seed {seed}: {}", m.ice_cover);
                assert_eq!(m.hydrosphere, 0.0, "seed {seed}");
                inner += 1;
            }
        }
        assert!(inner > 100, "{inner}");
    }

    #[test]
    fn outer_moons_of_a_jupiter_past_the_frost_line_stay_icy() {
        let (mut outer, mut iced) = (0, 0);
        for seed in 0..200 {
            let system = jupiter_system(seed);
            let jupiter = &system.planets[0];
            for m in regular(jupiter).filter(|m| !is_inner(m, jupiter)) {
                assert_eq!(m.orbit_zone, ICY_ZONE, "seed {seed}");
                assert!(!m.is_dry && !m.is_volcanic, "seed {seed}");
                assert_eq!(m.tidal_heating_w_m2, 0.0, "seed {seed}");
                outer += 1;
                iced += usize::from(m.ice_cover >= 0.5);
            }
        }
        assert!(outer > 100, "{outer}");
        assert!(iced > 0, "no outer moon kept its ice");
    }

    #[test]
    fn io_scales_to_ios_heat_and_closer_or_heavier_means_hotter() {
        let io_au = IO_DISTANCE_KM / KM_PER_AU;
        let io = tidal_heating_w_m2(JUPITER_SOLAR_MASSES, IO_RADIUS_KM, io_au);
        assert!((io - IO_HEATING_W_M2).abs() < 1e-9, "{io}");
        assert!(io >= VOLCANIC_MIN_W_M2);
        assert!(tidal_heating_w_m2(JUPITER_SOLAR_MASSES, IO_RADIUS_KM, 1.5 * io_au) < io);
        assert!(tidal_heating_w_m2(2.0 * JUPITER_SOLAR_MASSES, IO_RADIUS_KM, io_au) > io);
        let close = tidal_heating_w_m2(JUPITER_SOLAR_MASSES, IO_RADIUS_KM, 0.5 * io_au);
        assert_eq!(close, MAX_TIDAL_HEATING_W_M2);
    }

    #[test]
    fn only_inner_moons_with_an_outer_neighbour_are_heated_and_heat_warms_them() {
        let mut heated = 0;
        for seed in 0..200 {
            let system = jupiter_system(seed);
            let jupiter = &system.planets[0];
            let moons: Vec<_> = regular(jupiter).collect();
            for (i, m) in moons.iter().enumerate() {
                let outermost = i + 1 == moons.len();
                if m.tidal_heating_w_m2 > 0.0 {
                    assert!(is_inner(m, jupiter) && !outermost, "seed {seed}");
                    if m.is_volcanic {
                        let hot = rederived(m, m.tidal_heating_w_m2, &system, jupiter, seed);
                        let cold = rederived(m, 0.0, &system, jupiter, seed);
                        assert!(hot > cold, "seed {seed}: {hot} K, {cold} K unheated");
                    }
                    heated += 1;
                }
                assert_eq!(
                    m.is_volcanic,
                    m.tidal_heating_w_m2 >= VOLCANIC_MIN_W_M2,
                    "seed {seed}"
                );
            }
        }
        assert!(heated > 50, "{heated}");
    }

    fn rederived(
        moon: &Planetesimal,
        heating: f64,
        system: &System,
        planet: &Planetesimal,
        seed: u64,
    ) -> f64 {
        let mut moon = moon.clone();
        moon.tidal_heating_w_m2 = heating;
        let star = &system.primary_star;
        moon.derive_planetary_environment(
            &star.stellar_luminosity,
            &planet.mass,
            &star.main_seq_age,
            &star.ecosphere,
            &mut SplitMix64::new(seed),
        );
        moon.surface_temp_kelvin
    }

    #[test]
    fn the_same_seed_forms_the_same_family() {
        for seed in 0..20 {
            assert_eq!(jupiter_system(seed), jupiter_system(seed));
        }
    }
}
