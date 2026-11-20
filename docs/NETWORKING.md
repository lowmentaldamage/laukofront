# Networking (future design only — no code)

## Goal

2–6 human players over the internet, with AIs filling empty slots, without changing the rules.

## Model: server-authoritative lockstep

- The simulation is already deterministic: integer maths, in-house RNG, orders resolved in queue
  order, no frame-time dependence. Every machine running the same orders from the same seed ends
  in the same state.
- A **server** owns the match. Clients send `Order`s; the server validates them with
  `World::validate`, assigns them to a tick, and broadcasts per tick: `(tick, [orders])`.
- Every client runs `World::tick()` with exactly that order list. The server runs it too and is
  the authority.
- AIs run only on the server; their orders are broadcast like human ones.

## Timing

- Tick rate stays 10 Hz. Orders a client sends during tick N are scheduled for tick N + 2 (200 ms
  input delay) to hide latency up to ~150 ms.
- If a client has not received tick N's order list, it waits (does not guess).

## Desync detection

- Every 50 ticks each client sends a hash of `territories` + `players`; the server compares with
  its own. On mismatch the client receives a full state snapshot (the save format from Stage 13).

## Messages (draft)

| Direction | Message |
|---|---|
| C→S | `Join { name }`, `Order { order, client_tick }`, `Hash { tick, hash }` |
| S→C | `Welcome { player_id, seed, size, players }`, `Tick { tick, orders }`, `Snapshot { state }`, `GameOver { winner }` |

## Transport

UDP with a small reliability layer, or WebSockets for simplicity. To be decided when the work
starts; record in DECISIONS.md.

## Security

The server never trusts a client: every order is re-validated, players can only order for their
own `PlayerId`, rate limit of 20 orders per second per player.
