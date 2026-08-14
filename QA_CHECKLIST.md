# Manual QA Checklist

Covers visual/interactive behaviors not worth automating (see BUILD_DOC.md 2.11).
Run through this list before considering a release done. Automated coverage
(determinism, pathfinding validity, connectivity, congestion formula) lives in
`sim-core/tests/core_tests.rs` and runs with `cargo test`.

- [ ] Loading the app with no seed param generates a city and updates the URL with a seed.
- [ ] Reloading the same URL reproduces an identical city layout.
- [ ] Every zone block is reachable by road from every other zone block (visually spot-check a few routes).
- [ ] Agents visibly depart in a morning wave and an evening wave, not uniformly throughout the day.
- [ ] Roads visibly change color under heavy load and return to normal once load subsides.
- [ ] Play, pause, and both speed multipliers (2x, 4x) behave correctly.
- [ ] The time-of-day display accurately reflects simulation progress from 6 AM to 11 PM.
- [ ] The app sustains acceptable frame rate (no visible stutter) at the target concurrent agent count (100+ at rush hour).
- [ ] The deployed GitHub Pages URL loads and functions identically to local dev.
