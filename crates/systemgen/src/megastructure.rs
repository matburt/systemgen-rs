//! Megastructures: Dyson swarms and spheres, Matrioshka brains, ringworlds, Bishop rings,
//! orbital rings and Shkadov thrusters. They're rare and mostly the works of dead
//! civilizations. A living one needs a settled, high-tech mainworld in the system.
//!
//! This runs in two passes on its own stream:
//! * [`place`] runs before settlement. It picks the structures, sizes them from the star and
//!   host, and removes the solid bodies their builders took apart for material. A shell also
//!   dims the star as seen from outside it.
//! * [`commission`] runs after settlement and decides whether each one still works.
//!
//! A system that gets no structure draws nothing from the world stream it wouldn't
//! otherwise, so its planets and profiles are the same as with megastructures off.

use crate::model::{BodyKind, BodyRef, StarSystem};
use crate::rng::Rng;
use crate::star::SpectralClass;
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
    /// Chance that one in a system with a high-tech settled mainworld still works.
    pub active_chance: f64,
}

impl Default for Megastructures {
    fn default() -> Self {
        Self {
            chance: 0.04,
            second_chance: 0.15,
            active_chance: 0.5,
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

/// Picks, sizes and builds the system's megastructures; dims the star behind any shell.
pub(crate) fn place(system: &mut StarSystem, rules: &Megastructures, rng: &mut dyn Rng) {
    if !rng.chance(rules.chance) {
        return;
    }
    let mut count = 1;
    if rng.chance(rules.second_chance) {
        count += 1;
    }
    for _ in 0..count {
        let candidates = candidates(system);
        if candidates.is_empty() {
            return;
        }
        let kinds: Vec<MegaKind> = candidates.iter().map(|(k, _)| *k).collect();
        let weights: Vec<f64> = candidates.iter().map(|(_, w)| *w).collect();
        let kind = kinds[pick(&weights, rng)];
        let mega = build(system, kind, rng);
        system.megastructures.push(mega);
    }
}

/// Decides which structures still work, now that settlement is known.
pub(crate) fn commission(system: &mut StarSystem, rules: &Megastructures, rng: &mut dyn Rng) {
    let high_tech_home = system.mainworld.is_some_and(|m| {
        let w = &system.body(m).world;
        w.is_settled() && w.tech >= 12
    });
    for mega in &mut system.megastructures {
        let condition = if high_tech_home && rng.chance(rules.active_chance) {
            Condition::Active
        } else {
            match rng.below(6) {
                0..=2 => Condition::Derelict,
                3..=4 => Condition::Ruined,
                _ => Condition::Unfinished,
            }
        };
        mega.condition = condition;
        mega.era = (condition != Condition::Active).then(|| match rng.below(6) {
            0 => Era::Recent,
            1..=2 => Era::Old,
            _ => Era::Ancient,
        });
        if matches!(condition, Condition::Ruined | Condition::Unfinished) {
            mega.coverage *= rng.range_f64(0.1, 0.6);
        }
    }
    let blocked: f64 = system
        .megastructures
        .iter()
        .filter(|m| m.host == Host::Star)
        .map(|m| m.coverage)
        .fold(0.0, |left, c| left + (1.0 - left) * c);
    system.star.apparent_luminosity = system.star.luminosity * (1.0 - blocked);
}

/// What could stand in this system, with how often each is built.
fn candidates(system: &StarSystem) -> Vec<(MegaKind, f64)> {
    use MegaKind::*;
    let star = &system.star;
    let has = |k: MegaKind| system.megastructures.iter().any(|m| m.kind == k);
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
    if orbital_ring_host(system).is_some() {
        out.push((OrbitalRing, 25.0));
    }
    out
}

fn build(system: &mut StarSystem, kind: MegaKind, rng: &mut dyn Rng) -> Megastructure {
    let eco_au = system.star.luminosity.sqrt();
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
            mega.consumed_bodies = consume(system, 0.0, mega.radius * SHELL_CONSUMES_WITHIN);
        }
        MegaKind::Ringworld => {
            mega.width_km = RINGWORLD_WIDTH_KM;
            let span = mega.radius * RING_CONSUMES_WITHIN;
            mega.consumed_bodies = consume(system, mega.radius - span, mega.radius + span);
            let area = std::f64::consts::TAU * mega.radius * KM_PER_AU * mega.width_km;
            mega.habitable_area_earths = area / EARTH_SURFACE_KM2;
            mega.coverage = mega.width_km / (2.0 * mega.radius * KM_PER_AU);
        }
        MegaKind::BishopRing => {
            let (inner, outer) = system.star.habitable_zone_au;
            mega.radius = rng.range_f64(inner, outer);
            mega.width_km = BISHOP_WIDTH_KM;
            let area = std::f64::consts::TAU * BISHOP_RADIUS_KM * BISHOP_WIDTH_KM;
            mega.habitable_area_earths = area / EARTH_SURFACE_KM2;
        }
        MegaKind::OrbitalRing => {
            let host = orbital_ring_host(system).expect("candidates checked the host");
            let planet = system.body(host);
            mega.host = Host::Body(host);
            mega.radius = planet.radius_km + rng.range_f64(300.0, 2000.0);
            mega.width_km = rng.range_f64(5.0, 50.0);
        }
        MegaKind::ShkadovThruster => {
            mega.radius = eco_au * rng.range_f64(0.3, 0.8);
            mega.coverage = rng.range_f64(0.1, 0.4);
        }
    }
    mega
}

/// The largest solid planet big enough to ring, preferring settled or once-settled ones.
fn orbital_ring_host(system: &StarSystem) -> Option<BodyRef> {
    let taken = |r: BodyRef| {
        system
            .megastructures
            .iter()
            .any(|m| m.host == Host::Body(r))
    };
    system
        .bodies
        .iter()
        .enumerate()
        .map(|(i, b)| (BodyRef::planet(i), b))
        .filter(|(r, b)| b.kind.is_solid() && b.radius_km >= 3200.0 && !taken(*r))
        .max_by(|(_, a), (_, b)| a.radius_km.total_cmp(&b.radius_km))
        .map(|(r, _)| r)
}

/// Removes the solid planets (not gas giants or belts) orbiting between `lo` and `hi` AU,
/// renumbering structures already placed round the rest and dropping any whose host went.
fn consume(system: &mut StarSystem, lo: f64, hi: f64) -> u32 {
    let doomed: Vec<bool> = system
        .bodies
        .iter()
        .map(|b| {
            let solid = matches!(b.kind, BodyKind::Rocky | BodyKind::Icy | BodyKind::Dwarf);
            solid && (lo..=hi).contains(&b.orbit.distance)
        })
        .collect();
    let mut new_index = Vec::with_capacity(doomed.len());
    let mut next = 0;
    for &gone in &doomed {
        new_index.push((!gone).then_some(next));
        next += usize::from(!gone);
    }
    system.megastructures.retain_mut(|m| match &mut m.host {
        Host::Star => true,
        Host::Body(r) => match new_index[r.body] {
            Some(i) => {
                r.body = i;
                true
            }
            None => false,
        },
    });
    let mut flags = doomed.iter();
    system
        .bodies
        .retain(|_| !flags.next().copied().unwrap_or(false));
    doomed.iter().filter(|g| **g).count() as u32
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
