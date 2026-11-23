# QA checklist — v1.0.0

Run on 2026-11-23 (Windows 11, Ryzen 9 7950X, RTX 4090, 1440x900 window). Every item passed
unless noted.

## Automated

| Check | Result |
|---|---|
| `cargo test --release` | 153 passed, 0 failed |
| `cargo clippy --all-targets -- -D warnings` | clean |
| `cargo fmt --check` | clean |
| Nation matches, small map, 4 difficulties x 6 seeds | 24 of 24 end with a winner |
| Nation matches, medium map, hard, seeds 1-3 | 3 of 3 end with a winner (11-15 min) |
| Every map (4 styles, 6 Earth maps) | every match ends with a winner, except archipelago 5 of 6 |
| Team matches, 2-4 teams, seeds 1-2 | 6 of 6 end with a team winner |
| Save -> load replays to the same state hash | yes (unit test and in the game) |

## In the game

| Check | Result |
|---|---|
| Starting without options opens the main menu over a live bots-only match | yes |
| Play -> setup -> Start begins the chosen match, speed back to x1 | yes (`--menu-script`) |
| Esc pauses (game time stops), Esc / Resume continues | yes |
| Save game -> Main menu -> Continue saved game loads it | yes (`--menu-script`) |
| Settings are kept between runs | yes |
| Spawn, attack, boats, buildings, warships, nukes, diplomacy menu, end screen | yes (autoplay screenshots, P6-P11) |
| Packaged zip runs from a folder outside the project (config found next to the exe) | yes |
| No console window in the release build | yes |

## Performance (60 s autoplay, no vsync)

| Map | Players | Avg | FPS | p99 | Max | Frames > 33 ms |
|---|---|---|---|---|---|---|
| large generated | 50 bots + 10 nations | 2.51 ms | 399 | 6.08 ms | 8.66 ms | 0 |
| World | 50 + 16 | 2.42 ms | 413 | 5.04 ms | 9.56 ms | 0 |

## Known limitations

- The exe is not code-signed: SmartScreen or Smart App Control may block it (README.txt explains).
- One frame of about 80 ms when the first sound opens the audio device.
- Not measured on a slower machine (only the development PC is available); the engine uses
  0.2-0.4 ms per tick and frames 2-3 ms here, which leaves a large margin.
- Not ported (documented decisions): online multiplayer, Doomsday Clock, setup presets, music,
  upstream's full nation tactics (D40) and border-tile encirclement (D25, simplified).
- The notice reads "Copyright (c)" instead of the © sign: the built-in font has no ©.
