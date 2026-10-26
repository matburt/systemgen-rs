# Agent notes

`systemgen` makes plausible star systems for the space game (`../space`, Linear SPA-299). Each module's `//!` doc says what it owns: `star` → `accrete` → `moons` → `model` → `megastructure::place` → `world` → `megastructure::commission`; `lib.rs` wires them together.

## Rules

- **Deterministic.** Never use wall-clock time, OS randomness or `HashMap` iteration order. All randomness goes through `rng::Rng`. Each layer gets its own stream, split in `generate_with`. A change that moves any draw changes every system, so say so in the commit.
- **`accrete/` is vendored.** It's the MIT fork of LeonidGrr/accrete. Keep its notice and list every behaviour change in `accrete/mod.rs`. Clippy is allowed there; it is not allowed elsewhere.
- **Physics in, game out.** The library speaks AU, solar and Earth masses, km and K. Game units, content classes and scale belong in the game.
- **World rules are our own.** Keep the Traveller *shape* (the profile code, trade tags), but never copy published Traveller or Cepheus tables or text.

## Build and test

```bash
make check    # fmt, clippy -D warnings, tests: what CI runs
make survey   # statistics over 2,000 seeds; run it after tuning and quote the numbers
```

Tests guard what a game relies on: determinism, finite values, ordering, sparse settlement. Don't pin exact generated values.
