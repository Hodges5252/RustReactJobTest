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

## Realism & variety pass (UPDATE_DOC.md)

- [ ] Opposite-direction traffic on the same road renders as two visually separated lanes, not one overlapping line.
- [ ] Same-direction vehicles on a congested road visibly queue/space out rather than overlapping or passing through each other.
- [ ] At least a few genuine dead-end stub roads are visible and do not connect through to another intersection.
- [ ] The city layout includes at least one visibly larger merged block (spanning 2-3 grid cells) rather than every block being a uniform single square.
- [ ] Some vehicles are visibly active during the midday quiet period and during the late-evening period, not only during the two commute rush hours.
- [ ] Vehicles visibly pull out of a block onto the road when departing, and pull from the road into a block when arriving, rather than popping in/out exactly at an intersection corner.
- [ ] The legend correctly identifies each zone color and the congestion color ramp.
- [ ] All original `BUILD_DOC.md` manual QA checklist items still pass (no regressions).
