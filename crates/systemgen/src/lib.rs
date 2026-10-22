//! Plausible star systems from a seed, in four layers:
//!
//! 1. [`star`]: the primary star, picked first (class weights, then a mass), with its
//!    habitable zone and frost line.
//! 2. [`accrete`]: Dole/Sagan/Isaacson accretion for the planets, gas giants, moons and rings,
//!    then the Starform/StarGen environment pass for each body's surface. [`model`] turns the
//!    result into [`Body`]s and adds belts.
//! 3. [`world`]: a Traveller-shaped [`WorldProfile`] per body (starport, size, atmosphere,
//!    hydrographics, population, government, law, tech, trade tags, ruins), with sparse
//!    settlement.
//! 4. [`megastructure`]: rare Dyson swarms and spheres, ringworlds, habitat and orbital rings
//!    and stellar engines, mostly derelict.
//!
//! Generation is deterministic: the same seed and [`Config`] give the same system on a given
//! build and platform. Each layer draws from its own stream, split from the caller's, so a
//! game can drive generation from its own seeded [`Rng`] with [`generate_with`].
//!
//! [`generate_star`] gives just a seed's star, without the costly layers, for a map of many
//! systems. [`generate_where`] rerolls a seed until a system scores well enough, for a game
//! that needs a particular kind of system somewhere.

pub mod accrete;
pub mod megastructure;
pub mod model;
pub mod rng;
pub mod star;
pub mod world;

pub use accrete::Disk;
pub use megastructure::{Condition, Host, MegaKind, Megastructure, Megastructures};
pub use model::{
    Body, BodyKind, BodyRef, Orbit, RingBand, StarSystem, Surface, Zone, BELT_MAX_EARTH_MASSES,
    DWARF_MAX_RADIUS_KM,
};
pub use rng::{Rng, SplitMix64};
pub use star::{SpectralClass, Star, StarChoice, StarWeights};
pub use world::{Atmosphere, Era, Government, Remnant, Settlement, Starport, Tag, WorldProfile};

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Config {
    pub star: StarChoice,
    pub disk: Disk,
    /// Add a belt at the first outer gas giant's inner 2:1 resonance.
    pub resonance_belts: bool,
    pub settlement: Settlement,
    pub megastructures: Megastructures,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            star: StarChoice::default(),
            disk: Disk::default(),
            resonance_belts: true,
            settlement: Settlement::default(),
            megastructures: Megastructures::default(),
        }
    }
}

/// Generates a system from `seed` with the built-in [`SplitMix64`] stream.
pub fn generate(seed: u64, config: &Config) -> StarSystem {
    let mut system = generate_with(&mut SplitMix64::new(seed), config);
    system.seed = seed;
    system
}

/// Generates a system from a caller's stream. Draws four seeds from `rng`, one per layer.
pub fn generate_with(rng: &mut dyn Rng, config: &Config) -> StarSystem {
    let mut streams = LayerStreams::split(rng);

    let mass = star::pick_mass(&config.star, &mut streams.star);
    let accreted = accrete::planetary_system(mass, &config.disk, &mut streams.accrete);
    let star = Star::from_primary(&accreted.primary_star);
    let bodies = model::bodies(&accreted, &star, config.resonance_belts);

    let mut system = StarSystem {
        seed: 0,
        star,
        bodies,
        mainworld: None,
        megastructures: Vec::new(),
    };
    megastructure::place(
        &mut system,
        &config.megastructures,
        &mut streams.megastructure,
    );
    world::settle(&mut system, &config.settlement, &mut streams.world);
    megastructure::commission(
        &mut system,
        &config.megastructures,
        &mut streams.megastructure,
    );
    system
}

/// The star [`generate`] would give for `seed`, without accretion, worlds or megastructures.
///
/// Every field matches `generate(seed, config).star` except `apparent_luminosity`, which only
/// megastructures dim: here it always equals `luminosity`.
pub fn generate_star(seed: u64, config: &Config) -> Star {
    generate_star_with(&mut SplitMix64::new(seed), config)
}

/// The star [`generate_with`] would give from `rng`, as [`generate_star`] does for a seed.
/// Draws the same four seeds from `rng`, so the caller's stream ends where it would.
pub fn generate_star_with(rng: &mut dyn Rng, config: &Config) -> Star {
    let mut streams = LayerStreams::split(rng);
    let mass = star::pick_mass(&config.star, &mut streams.star);
    Star::from_primary(&accrete::PrimaryStar::new(mass))
}

/// Generates up to `tries` systems from sub-seeds of `seed` and returns the first whose
/// `score` reaches `good_enough`, or else the best-scoring one (the earliest on a tie), with
/// the sub-seed that made it. `generate(sub_seed, config)` gives the same system again.
///
/// Try 0 is `seed` itself; later sub-seeds hash `seed` with the try index. At least one try is
/// always made.
pub fn generate_where(
    seed: u64,
    config: &Config,
    tries: u32,
    score: impl Fn(&StarSystem) -> f64,
    good_enough: f64,
) -> (StarSystem, u64) {
    let mut best: Option<(StarSystem, u64, f64)> = None;
    for attempt in 0..tries.max(1) {
        let sub_seed = sub_seed(seed, attempt);
        let system = generate(sub_seed, config);
        let s = score(&system);
        if s >= good_enough {
            return (system, sub_seed);
        }
        if best.as_ref().is_none_or(|(_, _, b)| s > *b) {
            best = Some((system, sub_seed, s));
        }
    }
    let (system, sub_seed, _) = best.expect("at least one try");
    (system, sub_seed)
}

fn sub_seed(seed: u64, attempt: u32) -> u64 {
    if attempt == 0 {
        return seed;
    }
    let hashed_seed = SplitMix64::new(seed).next_u64();
    SplitMix64::new(hashed_seed ^ u64::from(attempt)).next_u64()
}

/// One stream per layer, split from the caller's in a fixed order.
struct LayerStreams {
    star: SplitMix64,
    accrete: SplitMix64,
    world: SplitMix64,
    megastructure: SplitMix64,
}

impl LayerStreams {
    fn split(rng: &mut dyn Rng) -> Self {
        Self {
            star: SplitMix64::new(rng.next_u64()),
            accrete: SplitMix64::new(rng.next_u64()),
            world: SplitMix64::new(rng.next_u64()),
            megastructure: SplitMix64::new(rng.next_u64()),
        }
    }
}
