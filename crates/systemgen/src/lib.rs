//! Plausible star systems from a seed, in three layers:
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

pub mod accrete;
pub mod megastructure;
pub mod model;
pub mod rng;
pub mod star;
pub mod world;

pub use accrete::Disk;
pub use megastructure::{Condition, Host, MegaKind, Megastructure, Megastructures};
pub use model::{Body, BodyKind, BodyRef, Orbit, RingBand, StarSystem, Surface, Zone};
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
    let mut star_rng = SplitMix64::new(rng.next_u64());
    let mut accrete_rng = SplitMix64::new(rng.next_u64());
    let mut world_rng = SplitMix64::new(rng.next_u64());
    let mut mega_rng = SplitMix64::new(rng.next_u64());

    let mass = star::pick_mass(&config.star, &mut star_rng);
    let accreted = accrete::planetary_system(mass, &config.disk, &mut accrete_rng);
    let star = Star::from_primary(&accreted.primary_star);
    let bodies = model::bodies(&accreted, &star, config.resonance_belts);

    let mut system = StarSystem {
        seed: 0,
        star,
        bodies,
        mainworld: None,
        megastructures: Vec::new(),
    };
    megastructure::place(&mut system, &config.megastructures, &mut mega_rng);
    world::settle(&mut system, &config.settlement, &mut world_rng);
    megastructure::commission(&mut system, &config.megastructures, &mut mega_rng);
    system
}
