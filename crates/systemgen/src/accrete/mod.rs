//! Physical layer: a vendored fork of [LeonidGrr/accrete](https://github.com/LeonidGrr/accrete)
//! (MIT, `LICENSE-accrete`). Dole's 1969 accretion model, as refined by Sagan and
//! Isaacson, plus the Starform/StarGen planetary environment pass. Distances are in AU,
//! masses in solar masses.
//!
//! How it differs from upstream:
//! * Randomness comes from [`crate::rng::Rng`], so a caller's seeded stream drives everything.
//! * No wasm bindings, event log, string ids or `rand` dependency.
//! * Post-accretion picks targets with [`utils::pick_weighted`], which tolerates a system with
//!   no planets.
//! * Moons take their temperature and gas loss from their distance to the star, not to their
//!   planet.
//! * Day length is in hours throughout, and the tidal-lock check compares a day with a year.
//! * A, B and O stars have a finite habitable zone.

#![allow(dead_code, clippy::all)]

pub mod consts;
mod dust;
mod enviro;
mod planetesimal;
mod primary_star;
mod ring;
mod system;
mod utils;

pub use planetesimal::Planetesimal;
pub use primary_star::PrimaryStar;
pub use ring::Ring;
pub use system::System;

use crate::rng::Rng;
// The modules reach each other's items through `use super::*`, as upstream's did.
#[allow(unused_imports)]
use {dust::*, planetesimal::*, primary_star::*, ring::*, system::*};

/// The dust cloud's parameters (Dole's constants).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Disk {
    /// Dole's "A"; 0.00125–0.0015 is his recommended range.
    pub dust_density_coeff: f64,
    /// Gas-to-dust ratio, 50–100.
    pub k: f64,
    /// 0.15–0.25; higher makes fewer planets.
    pub cloud_eccentricity: f64,
    /// Critical-mass coefficient for a planet to start taking gas: 1.0e-5–1.2e-5.
    pub b: f64,
    /// Late planetesimals thrown at the finished planets.
    pub post_accretion_intensity: u32,
}

impl Default for Disk {
    fn default() -> Self {
        Self {
            dust_density_coeff: consts::DUST_DENSITY_COEFF,
            k: consts::K,
            cloud_eccentricity: 0.2,
            b: consts::B,
            post_accretion_intensity: 1000,
        }
    }
}

/// Accretes a planetary system round a main-sequence star of `stellar_mass` solar masses.
pub fn planetary_system(stellar_mass: f64, disk: &Disk, rng: &mut dyn Rng) -> System {
    let mut system = System::set_initial_conditions(
        stellar_mass,
        disk.dust_density_coeff,
        disk.k,
        disk.cloud_eccentricity,
        disk.b,
    );
    system.distribute_planetary_masses(rng);
    system.post_accretion(disk.post_accretion_intensity, rng);
    system.process_planets(rng);
    system
}
