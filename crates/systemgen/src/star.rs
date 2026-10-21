//! The primary star, picked first; everything else derives from it. A spectral class is
//! chosen from weights, then a mass within that class. The class reported is always the one
//! the star's physics gives (the accrete layer's mass → temperature relation), so class mass
//! ranges are found by inverting that relation rather than taken from a table.

pub use crate::accrete::consts::SpectralClass;
use crate::accrete::PrimaryStar;
use crate::rng::Rng;

/// Hydrogen-burning limit; lighter bodies are brown dwarfs.
const MIN_STAR_MASS: f64 = 0.08;
/// Heavier stars are outside what the accretion model is meant for.
const MAX_STAR_MASS: f64 = 40.0;
/// Snow line of the solar nebula, scaled by √L.
const FROST_LINE_AU_AT_1_LSUN: f64 = 2.7;

/// Relative odds of each main-sequence class.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StarWeights {
    pub m: f64,
    pub k: f64,
    pub g: f64,
    pub f: f64,
    pub a: f64,
    pub b: f64,
    pub o: f64,
}

impl StarWeights {
    /// Roughly the solar neighbourhood: three in four stars are red dwarfs.
    pub const REALISTIC: Self = Self {
        m: 76.0,
        k: 12.0,
        g: 7.6,
        f: 3.0,
        a: 0.6,
        b: 0.13,
        o: 0.00003,
    };

    /// Tilted toward the K–F stars the accretion model handles best and players read as
    /// "a solar system", with enough red dwarfs and the odd hot star for variety.
    pub const GAME: Self = Self {
        m: 35.0,
        k: 30.0,
        g: 22.0,
        f: 10.0,
        a: 2.5,
        b: 0.5,
        o: 0.0,
    };

    fn table(&self) -> [(SpectralClass, f64); 7] {
        use SpectralClass::*;
        [
            (M, self.m),
            (K, self.k),
            (G, self.g),
            (F, self.f),
            (A, self.a),
            (B, self.b),
            (O, self.o),
        ]
    }
}

/// How the star is chosen.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum StarChoice {
    Weighted(StarWeights),
    Class(SpectralClass),
    /// Solar masses.
    Mass(f64),
}

impl Default for StarChoice {
    fn default() -> Self {
        Self::Weighted(StarWeights::GAME)
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Star {
    pub class: SpectralClass,
    /// Solar masses.
    pub mass: f64,
    /// Solar luminosities.
    pub luminosity: f64,
    /// What's left after Dyson swarms and shells, as seen from outside them.
    pub apparent_luminosity: f64,
    pub temperature_k: f64,
    pub radius_au: f64,
    /// Linear RGB, 0–1.
    pub color: [f64; 3],
    pub main_sequence_years: f64,
    /// Inner and outer edge, AU.
    pub habitable_zone_au: (f64, f64),
    pub frost_line_au: f64,
}

impl Star {
    pub(crate) fn from_primary(p: &PrimaryStar) -> Self {
        Self {
            class: p.spectral_class,
            mass: p.stellar_mass,
            luminosity: p.stellar_luminosity,
            apparent_luminosity: p.stellar_luminosity,
            temperature_k: p.stellar_surface_temp,
            radius_au: p.stellar_radius_au,
            color: p.color,
            main_sequence_years: p.main_seq_age,
            habitable_zone_au: p.ecosphere,
            frost_line_au: FROST_LINE_AU_AT_1_LSUN * p.stellar_luminosity.sqrt(),
        }
    }
}

/// Picks the star's mass in solar masses.
pub fn pick_mass(choice: &StarChoice, rng: &mut dyn Rng) -> f64 {
    match choice {
        StarChoice::Mass(m) => *m,
        StarChoice::Class(class) => mass_in_class(*class, rng),
        StarChoice::Weighted(weights) => {
            let table = weights.table();
            let total: f64 = table.iter().map(|(_, w)| w.max(0.0)).sum();
            let mut target = rng.range_f64(0.0, total);
            let mut class = SpectralClass::G;
            for (c, w) in table {
                let w = w.max(0.0);
                if target < w {
                    class = c;
                    break;
                }
                target -= w;
            }
            mass_in_class(class, rng)
        }
    }
}

/// Uniform in the class's mass range. Within a class, lighter stars are commoner, but the
/// class weights already carry that at the scale a game notices.
fn mass_in_class(class: SpectralClass, rng: &mut dyn Rng) -> f64 {
    let (lo, hi) = class_mass_range(class);
    rng.range_f64(lo, hi)
}

/// The masses whose physics lands in `class`, clamped to the main sequence we model.
pub fn class_mass_range(class: SpectralClass) -> (f64, f64) {
    use SpectralClass::*;
    let (t_lo, t_hi) = match class {
        M => (2400.0, 3700.0),
        K => (3700.0, 5200.0),
        G => (5200.0, 6000.0),
        F => (6000.0, 7500.0),
        A => (7500.0, 10000.0),
        B => (10000.0, 30000.0),
        O => (30000.0, f64::INFINITY),
        Rogue | Y | T | L => return (MIN_STAR_MASS, MIN_STAR_MASS),
    };
    let lo = mass_for_temperature(t_lo).max(MIN_STAR_MASS);
    let hi = if t_hi.is_finite() {
        mass_for_temperature(t_hi)
    } else {
        MAX_STAR_MASS
    };
    // A hair inside each edge so rounding never tips a star into the next class.
    let pad = (hi - lo) * 1.0e-6;
    (lo + pad, (hi - pad).min(MAX_STAR_MASS))
}

fn temperature_for_mass(mass: f64) -> f64 {
    PrimaryStar::new(mass).stellar_surface_temp
}

/// Bisection on the (monotonic) mass → temperature relation.
fn mass_for_temperature(target_k: f64) -> f64 {
    let (mut lo, mut hi) = (0.01_f64, 100.0_f64);
    for _ in 0..80 {
        let mid = (lo * hi).sqrt();
        if temperature_for_mass(mid) < target_k {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo * hi).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::SplitMix64;

    #[test]
    fn a_star_picked_by_class_has_that_class() {
        use SpectralClass::*;
        let mut rng = SplitMix64::new(3);
        for class in [M, K, G, F, A, B, O] {
            for _ in 0..200 {
                let mass = pick_mass(&StarChoice::Class(class), &mut rng);
                assert_eq!(PrimaryStar::new(mass).spectral_class, class, "{mass}");
            }
        }
    }

    #[test]
    fn the_sun_is_a_g_star_with_earth_in_its_habitable_zone() {
        let sun = Star::from_primary(&PrimaryStar::new(1.0));
        assert_eq!(sun.class, SpectralClass::G);
        let (inner, outer) = sun.habitable_zone_au;
        assert!(inner < 1.0 && 1.0 < outer, "{inner}–{outer}");
        assert!(outer < sun.frost_line_au);
    }

    #[test]
    fn weighted_picks_follow_the_weights() {
        let mut rng = SplitMix64::new(11);
        let only_k = StarWeights {
            m: 0.0,
            k: 1.0,
            g: 0.0,
            f: 0.0,
            a: 0.0,
            b: 0.0,
            o: 0.0,
        };
        for _ in 0..100 {
            let mass = pick_mass(&StarChoice::Weighted(only_k), &mut rng);
            assert_eq!(PrimaryStar::new(mass).spectral_class, SpectralClass::K);
        }
    }
}
