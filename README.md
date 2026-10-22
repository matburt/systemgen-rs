# systemgen-rs

Plausible star systems from a seed, for games. A Rust library (`systemgen`) and a small CLI.

Generation runs in four layers:

1. **Star.** A spectral class is picked from weights, then a mass within it. Luminosity, temperature, habitable zone and frost line follow from the mass.
2. **Accretion.** Dole's 1969 dust-cloud model, as refined by Sagan and Isaacson, forms the planets, gas giants, moons and rings. The Starform/StarGen environment pass then gives each body its gravity, pressure, temperature, water, ice and clouds. A belt is added at the first outer gas giant's 2:1 resonance. A body that hasn't cleared its orbit is a belt if under 0.001 Earth masses (between Ceres and Pluto) and a dwarf if under 1,500 km in radius; anything larger is icy or rocky by zone.
3. **World profile.** Each body gets a Traveller-shaped profile: starport, size, atmosphere, hydrographics, population, government, law, tech level and trade tags, plus ruins. The physical codes come from layer 2. Settlement is sparse: at most one settled mainworld per system, a few outposts round it, and ruins on worlds that could once have held life.
4. **Megastructures.** About one system in twenty-five has one: a Dyson swarm or sphere, a Matrioshka brain, a ringworld, a Bishop ring, an orbital ring or a Shkadov thruster. Each is sized from its star or host. Builders strip the solid planets a shell or ringworld needed, and shells dim the star as seen from outside. Most are derelict, ruined or unfinished; one still works only if the system has a settled high-tech mainworld.

Everything is deterministic: the same seed and `Config` always give the same system on a given build and platform. Each layer draws from its own stream, so a game can feed in its own seeded RNG.

## Use

```rust
use systemgen::{generate, Config, StarChoice, SpectralClass};

let system = generate(42, &Config::default());
println!("{:?} star, {} bodies", system.star.class, system.bodies.len());

// A G star, driven by your own RNG (implement `systemgen::Rng::next_u64`).
let config = Config { star: StarChoice::Class(SpectralClass::G), ..Config::default() };
let system = systemgen::generate_with(&mut my_rng, &config);

// Just the star, as `generate(42, &config).star` gives it, without the planets. Cheap enough
// for a map of thousands of systems. Only `apparent_luminosity` differs: megastructures dim
// it, and this skips them, so it equals `luminosity`.
let star = systemgen::generate_star(42, &config);

// Reroll seed 42 (up to 64 tries) until a score reaches 0.3; otherwise keep the best try.
// `sub_seed` regenerates the same system with `generate`. Try 0 is seed 42 itself.
let best_habitable_zone_world = |s: &systemgen::StarSystem| {
    s.all_bodies()
        .filter(|(_, b)| b.kind.is_solid() && b.orbit.zone == systemgen::Zone::Habitable)
        .map(|(_, b)| b.world.habitability)
        .fold(0.0, f64::max)
};
let (system, sub_seed) = systemgen::generate_where(42, &config, 64, best_habitable_zone_world, 0.3);
```

Habitable-zone worlds that good are scarce: about 8.5% of G systems and 1.7% of K systems have one, so 64 tries find one for nearly every G seed but only about 60% of K seeds; K needs about 256.

Enable the `serde` feature to serialize systems.

```text
$ cargo run -q -- 7
Seed 7: G star, 1.06 M☉, 1.225 L☉, 5738 K, habitable zone 0.93–1.84 AU, frost line 2.99 AU
        orbit  zone       kind       mass (E)    radius      g  temp K       bar  profile     tags
  1   0.32 AU  Hot        Rocky         0.093   2917 km   0.44     510     0.000  X400000-0   Va
  ...
```

The CLI takes `[SEED] [--class G | --mass 1.0] [--realistic] [--megastructure] [--star-only] [--json]`; `--megastructure` forces one and `--star-only` prints just the star line. `make survey` prints statistics over 2,000 seeds, which is useful when tuning.

## Units

The star and its planets use AU and solar masses. A body's mass is in Earth masses, its radius in km, temperatures in kelvin and pressure in bar. Moon orbits are in km round their planet. Converting to a game's own scale is up to the game.

## Development

```bash
make hooks   # once per clone: pre-commit runs fmt and clippy
make check   # fmt, clippy, tests (what CI runs)
make survey  # generation statistics
```

## Credits and licence

MIT, see `LICENSE`.

`crates/systemgen/src/accrete` is a fork of [LeonidGrr/accrete](https://github.com/LeonidGrr/accrete) (MIT, © 2020 Leonid Grebenshchikov, `LICENSE-accrete` alongside it). That in turn descends from Dole, from Sagan and Isaacson, from Fogg, Burdick and Burrows' StarGen. The fork's changes are listed in its module docs.

The world-profile layer follows the shape of a Traveller world profile but uses this crate's own rules and tables. It is not affiliated with Far Future Enterprises or Mongoose Publishing, and uses none of their content.
