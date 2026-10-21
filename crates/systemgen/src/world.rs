//! World layer: who lives where and what a place is good for, in the shape of a Traveller
//! world profile (starport, size, atmosphere, hydrographics, population, government, law,
//! tech, plus trade tags) but with this crate's own rules. Physical codes come straight from
//! the accrete layer. Social ones are rolled, driven by how habitable each body is.
//!
//! Settlement is sparse: a system has at most one settled mainworld, a few outposts if it
//! does, and ruins on uninhabited worlds that could once have held life.

use crate::model::{Body, BodyKind, BodyRef, StarSystem, Surface, Zone};
use crate::rng::Rng;

/// Size codes count 1,600 km of diameter each.
const KM_PER_SIZE_CODE: f64 = 1600.0;

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Settlement {
    /// Chance that a system's best world is settled, before habitability scales it.
    pub settled_chance: f64,
    /// Chance that each other body gets an outpost, when the mainworld is settled.
    pub outpost_chance: f64,
    /// Chance of ruins on an unsettled solid body of size 1 or more, before habitability
    /// scales it.
    pub ruin_chance: f64,
}

impl Default for Settlement {
    fn default() -> Self {
        Self {
            settled_chance: 0.3,
            outpost_chance: 0.15,
            ruin_chance: 0.2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Starport {
    A,
    B,
    C,
    D,
    E,
    #[default]
    None,
}

impl Starport {
    pub fn letter(self) -> char {
        match self {
            Starport::A => 'A',
            Starport::B => 'B',
            Starport::C => 'C',
            Starport::D => 'D',
            Starport::E => 'E',
            Starport::None => 'X',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Atmosphere {
    #[default]
    None,
    Trace,
    Thin,
    /// Earth-like pressure and temperature.
    Standard,
    Dense,
    /// Breathable pressure, wrong temperature or chemistry.
    Tainted,
    /// Hot and thick enough to be lethal.
    Toxic,
    /// Venus-like.
    Corrosive,
    /// A gas giant's envelope.
    Crushing,
}

impl Atmosphere {
    pub fn code(self) -> u8 {
        self as u8
    }

    pub fn of(kind: BodyKind, surface: Option<&Surface>) -> Self {
        if kind.is_gaseous() {
            return Atmosphere::Crushing;
        }
        let Some(s) = surface else {
            return Atmosphere::None;
        };
        let (p, t) = (s.pressure_bar, s.temperature_k);
        match p {
            p if p < 0.001 => Atmosphere::None,
            p if p < 0.1 => Atmosphere::Trace,
            p if p > 10.0 => Atmosphere::Corrosive,
            _ if t > 373.0 => Atmosphere::Toxic,
            p if p < 0.5 => Atmosphere::Thin,
            p if p <= 2.0 && (250.0..=320.0).contains(&t) => Atmosphere::Standard,
            p if p <= 2.0 => Atmosphere::Tainted,
            _ => Atmosphere::Dense,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Government {
    #[default]
    Unclaimed,
    CompanyTown,
    Cooperative,
    Council,
    Democracy,
    Technocracy,
    Bureaucracy,
    Oligarchy,
    Theocracy,
    Monarchy,
    Junta,
    Syndicate,
    Fractured,
}

impl Government {
    const ALL: [Government; 13] = [
        Government::Unclaimed,
        Government::CompanyTown,
        Government::Cooperative,
        Government::Council,
        Government::Democracy,
        Government::Technocracy,
        Government::Bureaucracy,
        Government::Oligarchy,
        Government::Theocracy,
        Government::Monarchy,
        Government::Junta,
        Government::Syndicate,
        Government::Fractured,
    ];

    pub fn code(self) -> u8 {
        self as u8
    }

    fn from_code(code: i32) -> Self {
        Self::ALL[code.clamp(0, Self::ALL.len() as i32 - 1) as usize]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Tag {
    Garden,
    Agricultural,
    Industrial,
    NonIndustrial,
    Rich,
    Poor,
    Ocean,
    Desert,
    IceCapped,
    Vacuum,
    Asteroid,
    GasGiant,
    HighPopulation,
    LowPopulation,
    HighTech,
    LowTech,
}

impl Tag {
    pub fn abbreviation(self) -> &'static str {
        match self {
            Tag::Garden => "Ga",
            Tag::Agricultural => "Ag",
            Tag::Industrial => "In",
            Tag::NonIndustrial => "Ni",
            Tag::Rich => "Ri",
            Tag::Poor => "Po",
            Tag::Ocean => "Oc",
            Tag::Desert => "De",
            Tag::IceCapped => "Ic",
            Tag::Vacuum => "Va",
            Tag::Asteroid => "As",
            Tag::GasGiant => "Gg",
            Tag::HighPopulation => "Hi",
            Tag::LowPopulation => "Lo",
            Tag::HighTech => "Ht",
            Tag::LowTech => "Lt",
        }
    }
}

/// How long ago a dead civilization fell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Era {
    Recent,
    Old,
    Ancient,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Remnant {
    pub era: Era,
}

#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WorldProfile {
    /// 0–1: how well unprotected people would fare here.
    pub habitability: f64,
    pub starport: Starport,
    /// Diameter in 1,600 km steps, 0 for belts and gas giants.
    pub size: u8,
    pub atmosphere: Atmosphere,
    /// Tenths of the surface under water, 0–10.
    pub hydrographics: u8,
    /// Order of magnitude of the population, 0 for none.
    pub population: u8,
    pub government: Government,
    pub law: u8,
    pub tech: u8,
    pub tags: Vec<Tag>,
    pub remnant: Option<Remnant>,
}

impl WorldProfile {
    /// The profile as one code, e.g. `C766542-9`.
    pub fn code(&self) -> String {
        format!(
            "{}{}{}{}{}{}{}-{}",
            self.starport.letter(),
            hex(self.size),
            hex(self.atmosphere.code()),
            hex(self.hydrographics),
            hex(self.population),
            hex(self.government.code()),
            hex(self.law),
            hex(self.tech),
        )
    }

    pub fn is_settled(&self) -> bool {
        self.population > 0
    }
}

fn hex(v: u8) -> char {
    char::from_digit(u32::from(v.min(35)), 36)
        .unwrap_or('?')
        .to_ascii_uppercase()
}

/// Fills every body's [`WorldProfile`] and picks the mainworld.
pub(crate) fn settle(system: &mut StarSystem, rules: &Settlement, rng: &mut dyn Rng) {
    let refs: Vec<BodyRef> = system.all_bodies().map(|(r, _)| r).collect();
    for &r in &refs {
        let body = body_mut(system, r);
        body.world = physical_profile(body);
    }

    system.mainworld = refs
        .iter()
        .copied()
        .max_by(|a, b| desirability(system.body(*a)).total_cmp(&desirability(system.body(*b))));

    let main_settled = system.mainworld.is_some_and(|main| {
        let h = system.body(main).world.habitability;
        rng.chance(rules.settled_chance * (0.5 + h))
    });

    for &r in &refs {
        let is_main = Some(r) == system.mainworld;
        let body = body_mut(system, r);
        let population = if is_main && main_settled {
            settled_population(body.world.habitability, rng)
        } else if !is_main && main_settled && rng.chance(rules.outpost_chance) {
            1 + rng.below(3) as u8
        } else {
            0
        };
        social_profile(body, population, !is_main, rng);
        if population == 0 && body.surface.is_some() && body.world.size >= 1 {
            body.world.remnant = ruins(body.world.habitability, rules.ruin_chance, rng);
        }
        body.world.tags = tags(body);
    }
}

fn body_mut(system: &mut StarSystem, r: BodyRef) -> &mut Body {
    let planet = &mut system.bodies[r.body];
    match r.moon {
        Some(m) => &mut planet.moons[m],
        None => planet,
    }
}

fn physical_profile(body: &Body) -> WorldProfile {
    let solid = body.kind.is_solid();
    WorldProfile {
        habitability: body.surface.as_ref().map_or(0.0, habitability),
        size: if solid {
            (body.radius_km * 2.0 / KM_PER_SIZE_CODE)
                .round()
                .clamp(0.0, 15.0) as u8
        } else {
            0
        },
        atmosphere: Atmosphere::of(body.kind, body.surface.as_ref()),
        hydrographics: body
            .surface
            .as_ref()
            .map_or(0, |s| (s.hydrosphere * 10.0).round() as u8),
        ..WorldProfile::default()
    }
}

/// 1 at Earth's temperature, pressure, gravity and a part-wet surface; 0 where nobody could
/// live unprotected.
pub fn habitability(s: &Surface) -> f64 {
    let temperature = tent(s.temperature_k, 245.0, 288.0, 325.0);
    let pressure = tent(s.pressure_bar.max(1.0e-6).log10(), -0.7, 0.0, 0.6);
    let gravity = tent(s.gravity_g, 0.25, 1.0, 2.0);
    let water = if (0.05..=0.95).contains(&s.hydrosphere) {
        1.0
    } else {
        0.4
    };
    let holds_air = if s.lightest_gas_retained <= 28.0 {
        1.0
    } else {
        0.0
    };
    temperature * pressure * gravity * water * holds_air
}

/// Rises linearly from `lo` to 1 at `peak`, falls to 0 at `hi`.
fn tent(x: f64, lo: f64, peak: f64, hi: f64) -> f64 {
    if !x.is_finite() || x <= lo || x >= hi {
        0.0
    } else if x <= peak {
        (x - lo) / (peak - lo)
    } else {
        (hi - x) / (hi - peak)
    }
}

/// Where people would settle first: livable worlds, then mineable belts and solid bodies
/// near the habitable zone.
fn desirability(body: &Body) -> f64 {
    let h = body.world.habitability;
    let near_home = matches!(body.orbit.zone, Zone::Habitable | Zone::Cool);
    let base = match body.kind {
        BodyKind::Belt => 0.12,
        BodyKind::GasGiant | BodyKind::IceGiant => 0.02,
        _ if near_home => 0.1,
        _ => 0.05,
    };
    h + base + f64::from(body.world.size) * 0.002
}

fn settled_population(habitability: f64, rng: &mut dyn Rng) -> u8 {
    // Domes and industry let a hostile mainworld grow; a garden world grows further.
    let roll = 3 + (habitability * 5.0).round() as i32 + rng.dice(2) - 7;
    roll.clamp(1, 10) as u8
}

fn social_profile(body: &mut Body, population: u8, outpost: bool, rng: &mut dyn Rng) {
    let w = &mut body.world;
    w.population = population;
    if population == 0 {
        return;
    }
    let pop = i32::from(population);
    w.government = Government::from_code(rng.dice(2) - 7 + pop);
    w.law = (rng.dice(2) - 7 + i32::from(w.government.code())).clamp(0, 12) as u8;

    let port_roll = rng.dice(2) + (pop - 5) / 2;
    w.starport = match port_roll {
        r if r >= 11 => Starport::A,
        r if r >= 9 => Starport::B,
        r if r >= 7 => Starport::C,
        r if r >= 5 => Starport::D,
        r if r >= 3 => Starport::E,
        _ => Starport::None,
    };
    if outpost && w.starport < Starport::D {
        w.starport = Starport::D;
    }

    let port_bonus = match w.starport {
        Starport::A => 6,
        Starport::B => 4,
        Starport::C => 2,
        Starport::D => 0,
        Starport::E | Starport::None => -1,
    };
    // A hostile world needs tech just to keep people alive.
    let harsh_bonus = if matches!(w.atmosphere, Atmosphere::Standard) {
        0
    } else {
        2
    };
    let pop_bonus = match population {
        9.. => 2,
        1..=3 => 1,
        _ => 0,
    };
    w.tech = (rng.dice(1) + port_bonus + harsh_bonus + pop_bonus).clamp(0, 15) as u8;
}

fn ruins(habitability: f64, ruin_chance: f64, rng: &mut dyn Rng) -> Option<Remnant> {
    if !rng.chance(ruin_chance * (0.25 + habitability)) {
        return None;
    }
    let era = match rng.below(6) {
        0 => Era::Recent,
        1..=2 => Era::Old,
        _ => Era::Ancient,
    };
    Some(Remnant { era })
}

fn tags(body: &Body) -> Vec<Tag> {
    let w = &body.world;
    let atmo = w.atmosphere;
    let hydro = w.hydrographics;
    let pop = w.population;
    let temperate = body
        .surface
        .as_ref()
        .is_some_and(|s| (260.0..=310.0).contains(&s.temperature_k));
    let breathable = matches!(atmo, Atmosphere::Standard | Atmosphere::Dense);

    let mut tags = Vec::new();
    let mut add = |on: bool, tag: Tag| {
        if on {
            tags.push(tag);
        }
    };
    add(
        atmo == Atmosphere::Standard && (4..=8).contains(&hydro) && temperate,
        Tag::Garden,
    );
    add(
        breathable && (4..=8).contains(&hydro) && (5..=7).contains(&pop),
        Tag::Agricultural,
    );
    add(pop >= 9 || (pop >= 8 && w.tech >= 10), Tag::Industrial);
    add((1..=6).contains(&pop), Tag::NonIndustrial);
    add(breathable && (6..=8).contains(&pop), Tag::Rich);
    add(
        pop > 0 && matches!(atmo, Atmosphere::Trace | Atmosphere::Thin) && hydro <= 3,
        Tag::Poor,
    );
    add(hydro == 10, Tag::Ocean);
    add(
        hydro == 0
            && !matches!(
                atmo,
                Atmosphere::None | Atmosphere::Trace | Atmosphere::Crushing
            ),
        Tag::Desert,
    );
    add(
        body.surface.as_ref().is_some_and(|s| s.ice_cover > 0.5),
        Tag::IceCapped,
    );
    add(
        atmo == Atmosphere::None && body.surface.is_some(),
        Tag::Vacuum,
    );
    add(body.kind == BodyKind::Belt, Tag::Asteroid);
    add(body.kind.is_gaseous(), Tag::GasGiant);
    add(pop >= 9, Tag::HighPopulation);
    add((1..=3).contains(&pop), Tag::LowPopulation);
    add(pop > 0 && w.tech >= 12, Tag::HighTech);
    add(pop > 0 && w.tech <= 5, Tag::LowTech);
    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    fn earthlike() -> Surface {
        Surface {
            gravity_g: 1.0,
            pressure_bar: 1.0,
            temperature_k: 288.0,
            day_temperature_k: 295.0,
            night_temperature_k: 280.0,
            hydrosphere: 0.7,
            ice_cover: 0.03,
            cloud_cover: 0.5,
            lightest_gas_retained: 5.0,
            greenhouse: false,
        }
    }

    #[test]
    fn earth_is_fully_habitable_and_standard() {
        let s = earthlike();
        assert!((habitability(&s) - 1.0).abs() < 1e-9);
        assert_eq!(
            Atmosphere::of(BodyKind::Rocky, Some(&s)),
            Atmosphere::Standard
        );
    }

    #[test]
    fn airless_or_scorched_worlds_are_uninhabitable() {
        let moon = Surface {
            pressure_bar: 0.0,
            hydrosphere: 0.0,
            ..earthlike()
        };
        assert_eq!(habitability(&moon), 0.0);
        assert_eq!(
            Atmosphere::of(BodyKind::Rocky, Some(&moon)),
            Atmosphere::None
        );

        let venus = Surface {
            pressure_bar: 92.0,
            temperature_k: 737.0,
            ..earthlike()
        };
        assert_eq!(habitability(&venus), 0.0);
        assert_eq!(
            Atmosphere::of(BodyKind::Rocky, Some(&venus)),
            Atmosphere::Corrosive
        );
    }

    #[test]
    fn profile_code_reads_like_a_world_profile() {
        let w = WorldProfile {
            starport: Starport::C,
            size: 7,
            atmosphere: Atmosphere::Standard,
            hydrographics: 7,
            population: 6,
            government: Government::Democracy,
            law: 4,
            tech: 9,
            ..WorldProfile::default()
        };
        assert_eq!(w.code(), "C737644-9");
    }
}
