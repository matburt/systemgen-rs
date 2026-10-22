//! Megastructures: Dyson swarms and spheres, Matrioshka brains, ringworlds, Bishop rings,
//! orbital rings and Shkadov thrusters. They're rare and mostly the works of dead
//! civilizations. A living one needs a settled, high-tech mainworld in the system.
//!
//! This runs in three passes on its own stream:
//! * [`plan`] runs from the star alone. It draws every structure's kind, size, condition, era
//!   and coverage, so the star's dimming (which [`dim`] applies) is known without planets:
//!   [`crate::generate_star`] runs only this pass.
//! * [`place`] runs before settlement and draws nothing. Shells and ringworlds remove the solid
//!   bodies their builders took apart for material; orbital rings take their planets.
//! * [`commission`] runs after settlement and decides whether each intact structure still
//!   works. A working one blocks the same light as a derelict one, so the dimming holds.
//!
//! A system that gets no structure draws nothing from the world stream it wouldn't
//! otherwise, so its planets and profiles are the same as with megastructures off.

use crate::model::{BodyKind, BodyRef, StarSystem};
use crate::rng::Rng;
use crate::star::{SpectralClass, Star};
use crate::world::Era;

const KM_PER_AU: f64 = 1.495_978_707e8;
const EARTH_SURFACE_KM2: f64 = 5.1e8;
/// Niven's ringworld: about a million miles wide.
const RINGWORLD_WIDTH_KM: f64 = 1.6e6;
/// Bishop's habitat ring.
const BISHOP_RADIUS_KM: f64 = 1000.0;
const BISHOP_WIDTH_KM: f64 = 500.0;
/// A shell's builders strip the solid bodies inside this many shell radii.
const SHELL_CONSUMES_WITHIN: f64 = 3.0;
/// A ringworld's builders strip the solid bodies within this fraction of its radius.
const RING_CONSUMES_WITHIN: f64 = 0.3;

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Megastructures {
    /// Chance that a system has a megastructure at all.
    pub chance: f64,
    /// Chance of a second one, given a first.
    pub second_chance: f64,
    /// Chance that an intact one in a system with a high-tech settled mainworld still works.
    pub active_chance: f64,
    /// The system's first structure is this kind, where its star allows one (else the system
    /// gets none); `None` picks by weight. For tools that want a particular structure.
    pub kind: Option<MegaKind>,
    /// The system's first structure is in this condition instead of a rolled one. `Active`
    /// still needs a high-tech settled mainworld: without one it's derelict.
    pub condition: Option<Condition>,
}

impl Default for Megastructures {
    fn default() -> Self {
        Self {
            chance: 0.04,
            second_chance: 0.15,
            active_chance: 0.5,
            kind: None,
            condition: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MegaKind {
    /// Collectors orbiting the star, catching part of its light.
    DysonSwarm,
    /// A complete shell round the star.
    DysonSphere,
    /// Nested shells, each one running on the waste heat of the one inside it: a star
    /// turned into a computer.
    MatrioshkaBrain,
    /// A ring round the star at Earth-like insolation, its inner face a continent-scale
    /// surface.
    Ringworld,
    /// A spun habitat ring a thousand km across.
    BishopRing,
    /// A ring round a planet, carrying elevators and lofstrom loops.
    OrbitalRing,
    /// A mirror on one side of the star that pushes the whole system.
    ShkadovThruster,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Condition {
    Active,
    /// Intact, no one home.
    Derelict,
    /// Broken.
    Ruined,
    /// Abandoned before it was finished.
    Unfinished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Host {
    Star,
    Body(BodyRef),
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Megastructure {
    pub kind: MegaKind,
    pub host: Host,
    /// From the host's centre: AU round the star, km round a body.
    pub radius: f64,
    pub width_km: f64,
    /// Share of the star's light it intercepts, 0–1.
    pub coverage: f64,
    /// Livable area in Earth surfaces.
    pub habitable_area_earths: f64,
    pub condition: Condition,
    /// When its builders fell, unless it's active.
    pub era: Option<Era>,
    /// Solid bodies dismantled to build it.
    pub consumed_bodies: u32,
}

/// A structure drawn from the star alone, before the system's bodies or settlement exist.
#[derive(Debug, Clone)]
pub(crate) struct Planned {
    mega: Megastructure,
    /// The condition was asked for in [`Megastructures::condition`], not rolled.
    forced: bool,
}

/// Draws every structure's kind, size, condition, era and coverage from the star alone.
///
/// Everything that dims the star is settled here, so [`crate::generate_star`] can dim it too.
/// An orbital ring's host and a shell's or ringworld's stripped bodies wait for [`place`]; an
/// intact structure only learns whether it still works in [`commission`], and working doesn't
/// change how much light it blocks.
pub(crate) fn plan(star: &Star, rules: &Megastructures, rng: &mut dyn Rng) -> Vec<Planned> {
    if !rng.chance(rules.chance) {
        return Vec::new();
    }
    let mut count = 1;
    if rng.chance(rules.second_chance) {
        count += 1;
    }
    let mut planned: Vec<Planned> = Vec::with_capacity(count);
    for slot in 0..count {
        let candidates = candidates(star, &planned);
        let weights: Vec<f64> = candidates.iter().map(|(_, w)| *w).collect();
        // Forcing draws what picking would, so forcing the kind it picks changes nothing.
        let picked = candidates[pick(&weights, rng)].0;
        let kind = match rules.kind.filter(|_| slot == 0) {
            None => picked,
            Some(kind) if candidates.iter().any(|(k, _)| *k == kind) => kind,
            Some(_) => break,
        };
        let mut mega = build(star, kind, rng);
        let rolled = match rng.below(6) {
            0..=2 => Condition::Derelict,
            3..=4 => Condition::Ruined,
            _ => Condition::Unfinished,
        };
        let forced = rules.condition.filter(|_| slot == 0);
        let condition = forced.unwrap_or(rolled);
        // A forced Active one stays intact until commission sees whether anyone runs it.
        mega.condition = match condition {
            Condition::Active => Condition::Derelict,
            c => c,
        };
        mega.era = Some(match rng.below(6) {
            0 => Era::Recent,
            1..=2 => Era::Old,
            _ => Era::Ancient,
        });
        if matches!(condition, Condition::Ruined | Condition::Unfinished) {
            mega.coverage *= rng.range_f64(0.1, 0.6);
        }
        planned.push(Planned {
            mega,
            forced: forced.is_some(),
        });
    }
    planned
}

/// Dims the star by the light its planned structures intercept.
pub(crate) fn dim(star: &mut Star, planned: &[Planned]) {
    let blocked: f64 = planned
        .iter()
        .filter(|p| p.mega.host == Host::Star)
        .map(|p| p.mega.coverage)
        .fold(0.0, |left, c| left + (1.0 - left) * c);
    star.apparent_luminosity = star.luminosity * (1.0 - blocked);
}

/// Builds the planned structures into the system: shells and ringworlds strip the solid
/// bodies in their way, then each orbital ring takes the largest solid planet left. An
/// orbital ring with no planet to ring is dropped; it blocks no starlight.
pub(crate) fn place(system: &mut StarSystem, planned: &mut Vec<Planned>) {
    for p in planned.iter_mut() {
        let mega = &mut p.mega;
        mega.consumed_bodies = match mega.kind {
            MegaKind::DysonSphere | MegaKind::MatrioshkaBrain => {
                consume(system, 0.0, mega.radius * SHELL_CONSUMES_WITHIN)
            }
            MegaKind::Ringworld => {
                let span = mega.radius * RING_CONSUMES_WITHIN;
                consume(system, mega.radius - span, mega.radius + span)
            }
            _ => 0,
        };
    }
    let mut ringed = Vec::new();
    planned.retain_mut(|p| {
        if p.mega.kind != MegaKind::OrbitalRing {
            return true;
        }
        let Some(host) = orbital_ring_host(system, &ringed) else {
            return false;
        };
        ringed.push(host);
        p.mega.host = Host::Body(host);
        p.mega.radius += system.body(host).radius_km;
        true
    });
}

/// Decides which intact structures still work, now that settlement is known, and adds them
/// all to the system.
pub(crate) fn commission(
    system: &mut StarSystem,
    planned: Vec<Planned>,
    rules: &Megastructures,
    rng: &mut dyn Rng,
) {
    let high_tech_home = system.mainworld.is_some_and(|m| {
        let w = &system.body(m).world;
        w.is_settled() && w.tech >= 12
    });
    for Planned { mut mega, forced } in planned {
        let intact_at_home = high_tech_home && mega.condition == Condition::Derelict;
        let rolled = intact_at_home && rng.chance(rules.active_chance);
        let works = intact_at_home
            && if forced {
                rules.condition == Some(Condition::Active)
            } else {
                rolled
            };
        if works {
            mega.condition = Condition::Active;
            mega.era = None;
        }
        system.megastructures.push(mega);
    }
}

/// What could stand round this star, with how often each is built. An orbital ring is
/// always a candidate; [`place`] drops it if no planet can hold it.
fn candidates(star: &Star, planned: &[Planned]) -> Vec<(MegaKind, f64)> {
    use MegaKind::*;
    let has = |k: MegaKind| planned.iter().any(|p| p.mega.kind == k);
    let shelled = has(DysonSphere) || has(MatrioshkaBrain);
    let main_sequence = matches!(
        star.class,
        SpectralClass::M
            | SpectralClass::K
            | SpectralClass::G
            | SpectralClass::F
            | SpectralClass::A
    );
    // Long-lived, steady stars only: a ringworld takes ages to build and to pay back.
    let steady = matches!(
        star.class,
        SpectralClass::K | SpectralClass::G | SpectralClass::F
    );

    let mut out = Vec::new();
    if main_sequence && !shelled {
        out.push((DysonSwarm, 30.0));
        if !has(DysonSwarm) {
            out.push((DysonSphere, 6.0));
            out.push((MatrioshkaBrain, 3.0));
        }
        if star.mass > 0.5 && !has(ShkadovThruster) {
            out.push((ShkadovThruster, 4.0));
        }
    }
    if steady && !has(Ringworld) && !shelled {
        out.push((Ringworld, 8.0));
    }
    out.push((BishopRing, 20.0));
    out.push((OrbitalRing, 25.0));
    out
}

/// Sizes a structure from the star. An orbital ring's radius is its altitude until [`place`]
/// finds its planet.
fn build(star: &Star, kind: MegaKind, rng: &mut dyn Rng) -> Megastructure {
    let eco_au = star.luminosity.sqrt();
    let mut mega = Megastructure {
        kind,
        host: Host::Star,
        radius: eco_au,
        width_km: 0.0,
        coverage: 0.0,
        habitable_area_earths: 0.0,
        condition: Condition::Derelict,
        era: None,
        consumed_bodies: 0,
    };
    match kind {
        MegaKind::DysonSwarm => {
            mega.radius = eco_au * rng.range_f64(0.3, 1.0);
            mega.coverage = rng.range_f64(0.02, 0.6);
        }
        MegaKind::DysonSphere | MegaKind::MatrioshkaBrain => {
            // A brain's outer shell sits well out, radiating near the cold of space.
            let at = if kind == MegaKind::DysonSphere {
                rng.range_f64(0.8, 1.2)
            } else {
                rng.range_f64(4.0, 8.0)
            };
            mega.radius = eco_au * at;
            mega.coverage = 1.0;
        }
        MegaKind::Ringworld => {
            mega.width_km = RINGWORLD_WIDTH_KM;
            let area = std::f64::consts::TAU * mega.radius * KM_PER_AU * mega.width_km;
            mega.habitable_area_earths = area / EARTH_SURFACE_KM2;
            mega.coverage = mega.width_km / (2.0 * mega.radius * KM_PER_AU);
        }
        MegaKind::BishopRing => {
            let (inner, outer) = star.habitable_zone_au;
            mega.radius = rng.range_f64(inner, outer);
            mega.width_km = BISHOP_WIDTH_KM;
            let area = std::f64::consts::TAU * BISHOP_RADIUS_KM * BISHOP_WIDTH_KM;
            mega.habitable_area_earths = area / EARTH_SURFACE_KM2;
        }
        MegaKind::OrbitalRing => {
            mega.radius = rng.range_f64(300.0, 2000.0);
            mega.width_km = rng.range_f64(5.0, 50.0);
        }
        MegaKind::ShkadovThruster => {
            mega.radius = eco_au * rng.range_f64(0.3, 0.8);
            mega.coverage = rng.range_f64(0.1, 0.4);
        }
    }
    mega
}

/// The largest solid planet big enough to ring that isn't ringed already.
fn orbital_ring_host(system: &StarSystem, ringed: &[BodyRef]) -> Option<BodyRef> {
    system
        .bodies
        .iter()
        .enumerate()
        .map(|(i, b)| (BodyRef::planet(i), b))
        .filter(|(r, b)| b.kind.is_solid() && b.radius_km >= 3200.0 && !ringed.contains(r))
        .max_by(|(_, a), (_, b)| a.radius_km.total_cmp(&b.radius_km))
        .map(|(r, _)| r)
}

/// Removes the solid planets (not gas giants or belts) orbiting between `lo` and `hi` AU.
fn consume(system: &mut StarSystem, lo: f64, hi: f64) -> u32 {
    let before = system.bodies.len();
    system.bodies.retain(|b| {
        let solid = matches!(b.kind, BodyKind::Rocky | BodyKind::Icy | BodyKind::Dwarf);
        !(solid && (lo..=hi).contains(&b.orbit.distance))
    });
    (before - system.bodies.len()) as u32
}

fn pick(weights: &[f64], rng: &mut dyn Rng) -> usize {
    let total: f64 = weights.iter().sum();
    let mut target = rng.range_f64(0.0, total);
    for (i, w) in weights.iter().enumerate() {
        if target < *w {
            return i;
        }
        target -= w;
    }
    weights.len() - 1
}
