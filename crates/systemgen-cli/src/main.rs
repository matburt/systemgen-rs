//! `systemgen [SEED] [--class G | --mass 1.0] [--realistic] [--megastructure] [--star-only] [--json]`:
//! prints one system, or with `--star-only` just its star.

use std::process::ExitCode;

use systemgen::{
    generate, generate_star, Body, BodyKind, Config, Host, SpectralClass, Star, StarChoice,
    StarSystem, StarWeights,
};

const USAGE: &str =
    "usage: systemgen [SEED] [--class O|B|A|F|G|K|M | --mass SOLAR_MASSES] [--realistic] [--megastructure] [--star-only] [--json]";

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    let mut seed = 1_u64;
    let mut config = Config::default();
    let mut json = false;
    let mut star_only = false;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" => json = true,
            "--star-only" => star_only = true,
            "--megastructure" => config.megastructures.chance = 1.0,
            "--realistic" => config.star = StarChoice::Weighted(StarWeights::REALISTIC),
            "--class" => {
                config.star = StarChoice::Class(parse_class(&value(&mut args, "--class")?)?)
            }
            "--mass" => {
                let mass = value(&mut args, "--mass")?;
                config.star =
                    StarChoice::Mass(mass.parse().map_err(|_| format!("bad mass {mass}"))?);
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            s => seed = s.parse().map_err(|_| format!("unknown argument {s}"))?,
        }
    }

    if star_only {
        let star = generate_star(seed, &config);
        if json {
            print_json(&star)?;
        } else {
            print_star(seed, &star);
            print_dimming(&star);
        }
        return Ok(());
    }
    let system = generate(seed, &config);
    if json {
        print_json(&system)?;
    } else {
        print_system(&system);
    }
    Ok(())
}

fn print_json(value: &impl serde::Serialize) -> Result<(), String> {
    let out = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    println!("{out}");
    Ok(())
}

fn value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next().ok_or_else(|| format!("{flag} needs a value"))
}

fn parse_class(s: &str) -> Result<SpectralClass, String> {
    use SpectralClass::*;
    Ok(match s.to_ascii_uppercase().as_str() {
        "O" => O,
        "B" => B,
        "A" => A,
        "F" => F,
        "G" => G,
        "K" => K,
        "M" => M,
        _ => return Err(format!("unknown class {s}")),
    })
}

fn print_star(seed: u64, s: &Star) {
    println!(
        "Seed {seed}: {:?} star, {:.2} M☉, {:.3} L☉, {:.0} K, habitable zone {:.2}–{:.2} AU, frost line {:.2} AU",
        s.class, s.mass, s.luminosity, s.temperature_k, s.habitable_zone_au.0, s.habitable_zone_au.1, s.frost_line_au
    );
}

fn print_dimming(s: &Star) {
    if s.apparent_luminosity < s.luminosity {
        println!(
            "  seen from outside: {:.3} L☉ of {:.3}",
            s.apparent_luminosity, s.luminosity
        );
    }
}

fn print_system(system: &StarSystem) {
    print_star(system.seed, &system.star);
    println!(
        "{:>3}  {:>8}  {:<9}  {:<8}  {:>9}  {:>8}  {:>5}  {:>6}  {:>8}  {:<10}  tags",
        "", "orbit", "zone", "kind", "mass (E)", "radius", "g", "temp K", "bar", "profile"
    );
    for (i, body) in system.bodies.iter().enumerate() {
        let main = system
            .mainworld
            .is_some_and(|m| m.body == i && m.moon.is_none());
        print_body(
            &format!("{}{}", if main { "*" } else { " " }, i + 1),
            body,
            "AU",
        );
        for (m, moon) in body.moons.iter().enumerate() {
            let main = system
                .mainworld
                .is_some_and(|r| r.body == i && r.moon == Some(m));
            print_body(
                &format!("{}{}{}", if main { "*" } else { " " }, i + 1, letter(m)),
                moon,
                "km",
            );
        }
    }
    for m in &system.megastructures {
        let at = match m.host {
            Host::Star => format!("{:.2} AU round the star", m.radius),
            Host::Body(r) => format!("{:.0} km round body {}", m.radius, r.body + 1),
        };
        let era = m.era.map_or(String::new(), |e| format!(", {e:?}"));
        println!(
            "  megastructure: {:?} ({:?}{era}), {at}, catches {:.0}% of starlight, {:.3} Earths of living space, {} bodies consumed",
            m.kind,
            m.condition,
            m.coverage * 100.0,
            m.habitable_area_earths,
            m.consumed_bodies
        );
    }
    print_dimming(&system.star);
}

fn letter(i: usize) -> char {
    char::from(b'a' + (i % 26) as u8)
}

fn print_body(label: &str, b: &Body, unit: &str) {
    let orbit = if unit == "AU" {
        format!("{:.2} AU", b.orbit.distance)
    } else {
        format!("{:.0}k km", b.orbit.distance / 1000.0)
    };
    let (g, temp, bar) = match &b.surface {
        Some(s) => (
            format!("{:.2}", s.gravity_g),
            format!("{:.0}", s.temperature_k),
            format!("{:.3}", s.pressure_bar),
        ),
        None => ("-".into(), "-".into(), "-".into()),
    };
    let radius = if b.kind == BodyKind::Belt {
        "-".to_string()
    } else {
        format!("{:.0} km", b.radius_km)
    };
    let mut tags: Vec<&str> = b.world.tags.iter().map(|t| t.abbreviation()).collect();
    if let Some(r) = b.world.remnant {
        tags.push(match r.era {
            systemgen::Era::Recent => "ruins (recent)",
            systemgen::Era::Old => "ruins (old)",
            systemgen::Era::Ancient => "ruins (ancient)",
        });
    }
    println!(
        "{label:>3}  {orbit:>8}  {:<9}  {:<8}  {:>9.3}  {radius:>8}  {g:>5}  {temp:>6}  {bar:>8}  {:<10}  {}",
        format!("{:?}", b.orbit.zone),
        format!("{:?}", b.kind),
        b.mass_earth,
        b.world.code(),
        tags.join(" ")
    );
}
