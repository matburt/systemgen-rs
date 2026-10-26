//! The public description of a generated system, and how the accrete layer's planetesimals
//! become it: units a game can use, zones relative to the star, belts, and moons in km. A
//! regular moon is `Rocky` or `Icy` by the composition it formed with, not by zone, and may be
//! `volcanic`.
//!
//! A top-level body that has not cleared its orbit is a belt if it's lighter than
//! [`BELT_MAX_EARTH_MASSES`], a dwarf if it's smaller than [`DWARF_MAX_RADIUS_KM`], and
//! otherwise `Icy` past the frost line or `Rocky` inside it, like any other solid body.

use crate::accrete::consts::{INCREDIBLY_LARGE_NUMBER, KM_PER_AU};
use crate::accrete::{Planetesimal, System};
use crate::megastructure::Megastructure;
use crate::star::Star;
use crate::world::WorldProfile;

/// Below this many Earth masses a body that has not cleared its orbit is a belt: between
/// Ceres (0.00016) and Pluto (0.0022).
pub const BELT_MAX_EARTH_MASSES: f64 = 0.001;
/// A body that has not cleared its orbit is a dwarf only below this radius (Pluto is 1,188
/// km, Mercury 2,440). A larger one reads as a planet and is `Icy` or `Rocky` by zone.
pub const DWARF_MAX_RADIUS_KM: f64 = 1500.0;
/// Gas giants clear a belt at their inner 2:1 resonance, (1/2)^(2/3) of their orbit.
const RESONANCE_2_1: f64 = 0.63;
/// A resonance belt needs this much room past the nearest inner body.
const BELT_CLEARANCE: f64 = 1.3;
/// Gaseous bodies lighter than this are ice giants (Uranus is 14.5).
const GAS_GIANT_MIN_EARTH_MASSES: f64 = 20.0;
/// Captured bodies lighter than this (about a 200 km rock) are dropped, not listed as moons.
const MOON_MIN_EARTH_MASSES: f64 = 1.0e-5;

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StarSystem {
    pub seed: u64,
    pub star: Star,
    /// Ordered outward.
    pub bodies: Vec<Body>,
    /// The system's most significant world, if anywhere is settled or worth settling.
    pub mainworld: Option<BodyRef>,
    pub megastructures: Vec<Megastructure>,
}

impl StarSystem {
    pub fn body(&self, at: BodyRef) -> &Body {
        let planet = &self.bodies[at.body];
        match at.moon {
            Some(m) => &planet.moons[m],
            None => planet,
        }
    }

    /// Every body and moon with its reference, planets before their moons.
    pub fn all_bodies(&self) -> impl Iterator<Item = (BodyRef, &Body)> {
        self.bodies.iter().enumerate().flat_map(|(i, planet)| {
            std::iter::once((BodyRef::planet(i), planet)).chain(
                planet
                    .moons
                    .iter()
                    .enumerate()
                    .map(move |(m, moon)| (BodyRef::moon(i, m), moon)),
            )
        })
    }
}

/// Index of a body, or of a moon of one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BodyRef {
    pub body: usize,
    pub moon: Option<usize>,
}

impl BodyRef {
    pub fn planet(body: usize) -> Self {
        Self { body, moon: None }
    }

    pub fn moon(body: usize, moon: usize) -> Self {
        Self {
            body,
            moon: Some(moon),
        }
    }
}

/// Where a body sits relative to its star's habitable zone and frost line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Zone {
    Hot,
    Habitable,
    /// Past the habitable zone, inside the frost line.
    Cool,
    /// Past the frost line.
    Outer,
}

impl Zone {
    pub fn of(star: &Star, distance_au: f64) -> Self {
        let (inner, outer) = star.habitable_zone_au;
        if distance_au < inner {
            Zone::Hot
        } else if distance_au <= outer {
            Zone::Habitable
        } else if distance_au < star.frost_line_au {
            Zone::Cool
        } else {
            Zone::Outer
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BodyKind {
    GasGiant,
    /// Gaseous, but too light for a gas giant.
    IceGiant,
    Rocky,
    /// A solid body formed past the frost line.
    Icy,
    /// Has not cleared its orbit and is smaller than [`DWARF_MAX_RADIUS_KM`].
    Dwarf,
    Belt,
}

impl BodyKind {
    pub fn is_gaseous(self) -> bool {
        matches!(self, BodyKind::GasGiant | BodyKind::IceGiant)
    }

    /// Has a surface you could stand on.
    pub fn is_solid(self) -> bool {
        !self.is_gaseous() && self != BodyKind::Belt
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Orbit {
    /// Semi-major axis: AU round the star, km round a planet.
    pub distance: f64,
    pub eccentricity: f64,
    /// The star distance's zone; a moon shares its planet's.
    pub zone: Zone,
}

/// Conditions at the surface of a solid body.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Surface {
    pub gravity_g: f64,
    pub pressure_bar: f64,
    pub temperature_k: f64,
    pub day_temperature_k: f64,
    pub night_temperature_k: f64,
    /// Fractions of the surface, 0–1.
    pub hydrosphere: f64,
    pub ice_cover: f64,
    pub cloud_cover: f64,
    /// The lightest molecule (g/mol) the body can hold on to: under 28 keeps nitrogen.
    pub lightest_gas_retained: f64,
    pub greenhouse: bool,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Body {
    pub kind: BodyKind,
    pub orbit: Orbit,
    pub mass_earth: f64,
    pub radius_km: f64,
    pub density_gcc: f64,
    /// None for gas giants and belts.
    pub surface: Option<Surface>,
    pub day_hours: f64,
    pub year_days: f64,
    pub axial_tilt_deg: f64,
    pub tidally_locked: bool,
    pub magnetosphere: bool,
    /// Accrete's broad flag: tidally locked, of 0.5–1.5 Earth masses, or struck in a collision.
    /// True of most solid bodies; see `volcanic` for active volcanism.
    pub tectonic: bool,
    /// Tidal heating drives Io-like volcanism ([`crate::moons`]). Only inner regular moons
    /// are heated, so it is false for planets and other moons.
    pub volcanic: bool,
    pub rings: Vec<RingBand>,
    pub moons: Vec<Body>,
    /// A moon formed in its giant's disk ([`crate::moons`]); false for captured moons and
    /// planets.
    pub regular_moon: bool,
    pub world: WorldProfile,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RingBand {
    pub distance_km: f64,
    pub width_km: f64,
}

/// The accrete layer's planets as [`Body`]s, outward, with belts added.
pub(crate) fn bodies(system: &System, star: &Star, add_resonance_belt: bool) -> Vec<Body> {
    let mut bodies: Vec<Body> = system.planets.iter().map(|p| body(p, star, None)).collect();
    if add_resonance_belt {
        insert_resonance_belt(&mut bodies, star);
    }
    bodies
}

fn body(p: &Planetesimal, star: &Star, parent: Option<&Planetesimal>) -> Body {
    let star_distance = parent.map_or(p.a, |planet| planet.a);
    let zone = Zone::of(star, star_distance);
    let kind = if p.is_gas_giant && p.earth_masses >= GAS_GIANT_MIN_EARTH_MASSES {
        BodyKind::GasGiant
    } else if p.is_gas_giant {
        BodyKind::IceGiant
    } else if parent.is_none() && p.is_dwarf_planet && p.earth_masses < BELT_MAX_EARTH_MASSES {
        BodyKind::Belt
    } else if parent.is_none() && p.is_dwarf_planet && p.radius < DWARF_MAX_RADIUS_KM {
        BodyKind::Dwarf
    } else if p.is_regular_moon {
        if p.orbit_zone == 1 {
            BodyKind::Rocky
        } else {
            BodyKind::Icy
        }
    } else if zone == Zone::Outer {
        BodyKind::Icy
    } else {
        BodyKind::Rocky
    };
    let distance = match parent {
        Some(_) => p.a * KM_PER_AU,
        None => p.a,
    };
    let solid = kind.is_solid();
    let surface = solid.then(|| Surface {
        gravity_g: p.surface_grav,
        pressure_bar: finite_or_zero(p.surface_pressure_bar),
        temperature_k: finite_or_zero(p.surface_temp_kelvin),
        day_temperature_k: p.day_temp_kelvin,
        night_temperature_k: p.night_temp_kelvin,
        hydrosphere: finite_or_zero(p.hydrosphere).clamp(0.0, 1.0),
        ice_cover: p.ice_cover.clamp(0.0, 1.0),
        cloud_cover: p.cloud_cover.clamp(0.0, 1.0),
        lightest_gas_retained: p.molecule_weight,
        greenhouse: p.greenhouse_effect,
    });
    Body {
        kind,
        orbit: Orbit {
            distance,
            eccentricity: p.e,
            zone,
        },
        mass_earth: p.earth_masses,
        radius_km: p.radius,
        density_gcc: p.density,
        surface,
        day_hours: p.day_hours,
        year_days: p.orbital_period_days,
        axial_tilt_deg: p.axial_tilt,
        tidally_locked: p.is_tidally_locked,
        magnetosphere: p.magnetosphere,
        tectonic: p.tectonic_activity,
        volcanic: p.is_volcanic,
        rings: p
            .rings
            .iter()
            .map(|r| RingBand {
                distance_km: r.a * KM_PER_AU,
                width_km: r.width,
            })
            .collect(),
        moons: if kind == BodyKind::Belt {
            Vec::new()
        } else {
            let mut moons: Vec<Body> = p
                .moons
                .iter()
                .filter(|m| is_real_moon(m))
                .map(|m| body(m, star, Some(p)))
                .collect();
            moons.sort_by(|a, b| a.orbit.distance.total_cmp(&b.orbit.distance));
            moons
        },
        regular_moon: p.is_regular_moon,
        world: WorldProfile::default(),
    }
}

fn is_real_moon(m: &Planetesimal) -> bool {
    m.earth_masses >= MOON_MIN_EARTH_MASSES
        && m.radius > 0.0
        && m.a.is_finite()
        && m.surface_grav.is_finite()
}

fn finite_or_zero(v: f64) -> f64 {
    if v.is_finite() && v < INCREDIBLY_LARGE_NUMBER {
        v
    } else {
        0.0
    }
}

/// A belt at the inner 2:1 resonance of the first gas giant past the frost line, if nothing
/// already sits there (Sol's main belt and Jupiter).
fn insert_resonance_belt(bodies: &mut Vec<Body>, star: &Star) {
    let Some(giant) = bodies
        .iter()
        .position(|b| b.kind.is_gaseous() && b.orbit.distance >= star.frost_line_au)
    else {
        return;
    };
    let at = bodies[giant].orbit.distance * RESONANCE_2_1;
    let inner_neighbour = bodies[..giant].last().map_or(0.0, |b| b.orbit.distance);
    if at < inner_neighbour * BELT_CLEARANCE
        || bodies[..giant].iter().any(|b| b.kind == BodyKind::Belt)
    {
        return;
    }
    let belt = Body {
        kind: BodyKind::Belt,
        orbit: Orbit {
            distance: at,
            eccentricity: 0.0,
            zone: Zone::of(star, at),
        },
        mass_earth: 0.0005,
        radius_km: 0.0,
        density_gcc: 2.0,
        surface: None,
        day_hours: 0.0,
        year_days: (at.powi(3) / star.mass).sqrt() * 365.256,
        axial_tilt_deg: 0.0,
        tidally_locked: false,
        magnetosphere: false,
        tectonic: false,
        volcanic: false,
        rings: Vec::new(),
        moons: Vec::new(),
        regular_moon: false,
        world: WorldProfile::default(),
    };
    bodies.insert(giant, belt);
}
