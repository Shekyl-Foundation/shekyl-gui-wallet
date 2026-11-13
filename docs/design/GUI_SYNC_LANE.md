# GUI sync lane — `GS-`

**Status:** LIVING CONTRACT — the GUI's contract with shekyl-core's wire,
artifacts and rulings. Round 0 read at core `dev` `32cfa46cf` (merge of
#1150) and `feat/tsr-inc3-lifecycle` `bb774a6` (#1151), GUI `dev`
`479fa1e` (merge of #45) and `docs/tor-installer-and-status` `1cc7e91`
(#47). `GS-Q1`…`GS-Q5` ruled by Rick 2026-10-22, `GS-Q6` 2026-11-08. Family
`GS-` (`GS-F` findings, `GS-Q` questions, `GS-0`… GUI increments, `GS-C1`…
core increments) is registered in core's `IMPLEMENTATION_INDEX.md` §2.
Code anchors below resolve at the read SHAs.

**Charter.** The GUI is the primary wallet for most users. This lane keeps it
on core's current contract: it consumes core's wire, artifacts and rulings
as they land, and it never carries a private copy of a core fact. The first
slice is Tor, because the GUI today ships a daemon whose transactions can
sit held with nothing said.

---

## 1. Ground truth

| Fact | Anchor |
| --- | --- |
| The GUI's installers carry `shekyld` and no tor. The release job builds `shekyld` on the runner with make/cmake and copies it into `src-tauri/binaries/`; nothing stages `tor/` or `tor-licenses/` | GUI `.github/workflows/release.yml:160-250`; `tauri.conf.json` `externalBin` is `shekyld` only |
| The sidecar is spawned view-only on plaintext loopback: `--rpc-admin-listen none --rpc-view-listen 127.0.0.1:<port>`. No `tor`, `clearnet` or `tor-file` argument | GUI `src-tauri/src/daemon_manager.rs:115-124` |
| The view preset has no Status part and refuses the admin paths, mining among them | core `rust/shekyl-daemon-rpc/src/server.rs:658-669` (`SPECIFIED_ADMIN_ONLY_PATHS`); GUI `daemon_rpc.rs:298-344` calls `/start_mining`, `/stop_mining`, `/mining_status` |
| No same-user socket or pipe exists in the daemon. RT-15 is ruled; RT-W10, which builds it, is "not authorized" | core `RPC_CHANNEL.md` §13; no `UnixListener` or named-pipe server in `shekyl-daemon-rpc` |
| The supervisor's typed read (`Status`, `OwnTx`, `ManagedTorFix`, `Cause`, `render`) exists only on the increment-3 branch, in-process, in `shekyl-tor-control-client`. Nothing serves it on the RPC wire; `InfoHealth` has no Tor boolean | core `feat/tsr-inc3-lifecycle` `rust/shekyl-tor-control-client/src/supervisor/posture.rs`; `dev` `rust/shekyl-rpc-types/src/info.rs:114-133` |
| The supervisor's Health is the link: `health = shown == Usable` | branch `supervisor/core/mod.rs:379` |
| The relay holds an own transaction when the hidden-address session class is empty, or the epoch's pin is exhausted, and the pool retries on its short grid | core `rust/shekyl-relay/src/graph/mod.rs:273-278`, `hidden_slot.rs:20-24`; `src/cryptonote_protocol/levin_notify.cpp:1211-1219` |
| The GUI reports a submit verdict of `Accepted` as "Transaction submitted." | GUI `src/pages/Send.tsx:61-64` |
| Stop: `SIGTERM`, sleep 2 s, `SIGKILL` unconditionally, by raw PID; Windows `taskkill /F` at once | GUI `daemon_manager.rs:219-235` |
| A clean stop is what writes Tor's directory cache back (`TSR-Q8`); a kill writes nothing | core `TOR_SUPERVISOR.md` §1 `TSR-Q8`, §12 increment 3b |
| Start: anything answering HTTP on the port is taken as an external daemon; "ready" is any HTTP reply; `height: 0` is hard-coded | GUI `daemon_manager.rs:92-99`, `188-199`, `211-217` |
| The GUI has its own reply types for the daemon wire | GUI `daemon_rpc.rs` (contra core RK-D1, `RPC_WIRE.md` *Carried decisions*) |
| Core records the GUI as unable to decode `get_info` on dev | core `docs/FOLLOWUPS.md` ("The GUI wallet cannot decode the daemon's `get_info` reply"), `DAEMON_RPC_KV_GET_INFO.md` §5.1 |
| The GUI repo has zero Tor, onion or clearnet code or UI | `rg -i 'tor\b|onion|clearnet' src src-tauri/src` |

## 2. Findings

**GS-F1 (high). The GUI's daemon has no Tor.** TB-2 makes it start, hold
its own transactions and announce why, into a log the GUI demotes to
`debug`/`warn` under `target: "shekyld"` and never shows. Every GUI user
today is in the `RestoreManaged` row of the posture table without knowing
it.

**GS-F2 (high). A held send is reported as sent.** `Accepted` is true (the
transaction is in the node's pool) and the pool will retry, but the person
reads "submitted" and walks away while the transaction waits for a Tor
that, per GS-F1, is not there. This is the silent hold.

**GS-F3 (high, core). `OwnTx::AddressHiding` can be shown while the relay
is holding.** `own_tx` reads the link (`Shown::Usable`). The relay holds on
a different fact: no outbound session that hides our address
(`HiddenClass::Empty`), or an exhausted epoch pin. After a Tor boot the
link is usable seconds before any onion peer is handshaken. Two sources for
one fact ("can this node originate now"), and the projection reads the
wrong one. `TSR-Q4`'s own wording, "this node can originate over an
address-hiding connector now", is the relay's fact, not the link's. Ruled a
defect (`GS-Q3`).

**GS-F4 (high, core + GUI). No path carries Status to the GUI.** Three
links are missing:
1. *Serve*: nothing puts the supervisor's read on the RPC wire.
2. *Name*: the RW round names the fields, in Part C, landing with the
   post-flip rename.
3. *Reach*: the GUI's leg is the view listener, which has no Status.
   RT-15's same-user socket or pipe is RT-W10's to build and is not
   authorized. Core's `TSR-Q12`, `TOR_SUPERVISOR.md` §11 and its FOLLOWUPS
   row for the RW naming are corrected to say so (`GS-C4`).

**GS-F5 (medium-high). Every GUI stop is a kill, so every GUI boot is a
cold Tor bootstrap.** 2 s then `SIGKILL` (and `taskkill /F` on Windows)
means the directory cache is never written back. The GUI user, the one who
most needs a fast start, gets the floor device's cold tail every time (p99
127 s, max 381 s, `TSR-M1`) instead of the seeded start of a few seconds.
The raw-PID kill can also hit an unrelated process if the daemon already
exited and the PID was reused. On Windows a console-less child receives no
ctrl event, so the daemon has no graceful trigger at all from the GUI. If
the GUI crashes, the daemon is orphaned holding the data-home lock.

**GS-F6 (medium). The start is a ladder.** "Something answered on the port,
so it is external" is the fallback RT-15 forbids: a stale daemon, another
network's daemon or any HTTP service becomes the GUI's node, silently.
Readiness and `height: 0` are sentinels.

**GS-F7 (medium). The fixes are not actionable.** The posture table offers
`Operator`, `ClearnetOn`, `ClearnetOnly`. The GUI has no Tor or clearnet
setting and does not own a config for its daemon, so even a rendered
message would name acts the person cannot take.

**GS-F8 (medium). Wire debt.** Own reply types (RK-D1); `get_info`
undecodable on dev; the Mining page calls admin-only paths on a view-only
listener (dead in the shipped alpha.10 GUI); the health probe uses REST
`GET /get_info`, which RW-Q2 retires; the RW rename is "paired with the GUI
repository's change" and the GUI is not positioned to pair.

## 3. Questions and rulings (Rick, 2026-10-22)

**GS-Q1 — How does the GUI reach its own daemon's owner grant?** Status,
the host-only stop, and mining all need the owner grant. The only leg that
gives it, RT-W10's same-user socket or pipe with its rendezvous, is unbuilt
and unauthorized; re-enabling the plaintext admin listener hands every
local account mining, bans and stop (why GUI #41 removed it).
*Ruled:* the owning connection first, then RT-W10. The GUI starts `shekyld`
with one end of a connected socket pair (an inherited handle on Windows) as
its owner connection, the mechanism the daemon already uses for Tor
(`TSR-Q7`, `__OwningControllerFD`): authenticated by construction, no path
and no rendezvous to squat, the owner grant, and closing it is the clean
stop on every platform, Windows included; parent death closes it, so a
crashed GUI cannot orphan the daemon. It is RT-15's same-user leg with
inheritance instead of a rendezvous. RT-W10's same-user leg follows for
every other same-user client (`shekyl-cli` beside a GUI-run node; a node
that outlives the GUI), queued in the RPC lane. Core increment `GS-C3`.

**GS-Q2 — What does "keep the daemon running after the wallet closes"
become?** Under the owning connection the sidecar's life is the GUI's; a
node that outlives the GUI is a run-as-me or service node (RT-16), found
through its rendezvous.
*Ruled:* the setting stays where it is and answers "not yet implemented"
loudly until RT-W10 lands.

**GS-Q3 — Which fact does `OwnTx` project (GS-F3)?**
*Ruled:* a defect, not a question. `OwnTx` and Health project the relay's
own hop-0 verdict for a local origin (hidden class non-empty and the epoch
pin not exhausted); nothing else is the fact. Fixed in core (`GS-C1`); the
link stays the cause and the phase.

**GS-Q4 — Who serves Status on the wire, and when relative to the RW
rename?** The rename lands after the flip; the GUI cannot wait for it to
stop the silent hold.
*Ruled:* core serves the supervisor read now as the Tor part of `get_info`'s
Status (owner grant) and Health's one boolean, with the posture types and
`render` moved out of `shekyl-tor-control-client` into `shekyl-rpc-types`
(RK-D1: the wire's one definition; the GUI does not link a Tor launcher to
get words). Built to RW-Q2…RW-Q4's conventions now; the names enter Part C's
census and the rename renames them like everything else. The GUI's
`get_info` decode waits for the daemon exactly as the CLI's does; that is
not a GUI defect. Core increment `GS-C2`.

**GS-Q5 — Where do the GUI's daemon and Tor come from?** The release job
rebuilds `shekyld` on GitHub runners, outside gitian, so the GUI ships a
different binary from core's tag and skips TB-6/TB-7/TB-13.
*Ruled:* the GUI's daemon and Tor come from gitian, like every build: the
installers consume core's gitian release archive for each target
(`shekyld`, `tor/`, `tor-licenses/`, staged and pin-verified by
`tor_bundle.py`) and verify its digest before unpacking. The Engine
path-deps still compile against the same core tag.

**GS-Q6 — How is a signed Tor made available to the GUI's installers, and
what form does the macOS pin take?** Raised by `GS-Q5`'s ruling: the Expert
Bundle is not code-signed on macOS or Windows.
*Ruled (Rick, 2026-11-08: "it ships with a signed macOS TOR"):* every
installer carries Tor, the `.dmg` included, and the signed Tor is core's
TB-11 (shekyl-core #1167). Both macOS targets are pinned at 15.0.26 with the
Foundation's Developer ID-signed `tor` and `libevent` (release
`tor-macos-15.0.26-signed`). Each macOS row of `config/tor_pins.json` holds
two digest sets: `files`, the signed bytes that ship and that the daemon
checks at every spawn, and `upstream`, Tor's own, which `tor_bundle.py` ties
to them. `gitian-osx.yml` stages the signed files into the macOS archive and
signs nothing. The GUI takes `tor/` from that archive and never re-signs it:
re-signing changes the bytes, and the pin refuses them. Linux and Windows
carry Tor's own bytes as pinned.

## 4. Design for the slice

- **Tor in the installers (TB-12, GS-F1).** Each installer carries core's
  `tor/` (exactly the pinned files: TB-7 part 3 allows nothing else in that
  directory, so it is its own Tauri resource directory) and
  `tor-licenses/`. The GUI passes `tor-file` naming the resolved resource
  path explicitly, one mechanism on every platform, rather than relying on
  beside-the-executable (Tauri puts `externalBin` and resources in
  different places per platform). The pin is still checked at every spawn.
  Every platform, macOS included (`GS-Q6`): the `.dmg` takes the signed
  `tor/` from core's macOS archive as it is.
- **One target, one stop (GS-F5, GS-F6).** The GUI's target is "this
  computer" or an explicit node, never a probe ladder. The sidecar is held
  by its child handle, never a raw PID. Stop is the owning connection's
  close (`GS-Q1`), then a wait bounded by core's derived stop timeout; a
  kill only after that bound, reported.
- **Posture shown, never reworded (TSR-Q12, TSR-Q13).** The GUI reads
  Status on its normal refresh and shows `render(status)` verbatim: a
  persistent posture line in the header whenever `OwnTx` is not
  `AddressHiding`, and the full read in Settings → Network. Against a node
  whose grant withholds Status, it says that it cannot see the node's Tor
  state; it does not show an empty posture.
- **Held is a state, not a moment (GS-F2).** Before submit, the review card
  reads `OwnTx`; on `Held` it states the transaction will wait on this node
  until Tor carries it and will not go out over clearnet, and the person
  confirms or waits. After submit, the verdict line says "accepted by your
  node", not "submitted". Every pending own transaction is shown as held
  for as long as `OwnTx` is `Held`, so a Tor that drops after the check is
  still told. `ClearnetOnly` sends say they went over the open network;
  `Offline` is not sent.
- **The fixes are the person's acts (GS-F7, TSR-Q11).** Settings → Network
  offers exactly the `ManagedTorFix` values in the read, each with the
  renderer's consequence. Taking one writes the sidecar's daemon config
  (TOML, the daemon-config slice's file) and restarts it. Nothing is
  pre-selected; `ClearnetOnly` is never a default, a one-click "just work"
  button, or something the GUI does on a timer.
- **Wire debt (GS-F8).** The GUI's reply types are deleted for
  `shekyl-rpc-types` through `shekyl-rpc-client`; mining moves to the owner
  leg; the GUI tracks core `dev` in CI and pairs with the RW rename PR.

## 5. Increments

| # | Repo | Contents | Blocked on |
| --- | --- | --- | --- |
| GS-0 | GUI | This charter; the two Tor rows of GUI #47 replaced by this lane's rows | — |
| GS-C1 | core | `GS-Q3`: `OwnTx` and Health from the relay's hop-0 verdict (shekyl-core #1164) | increment 3 (#1151) |
| GS-C2 | core | `GS-Q4`: Status' Tor part and Health's boolean on `get_info`; posture types and `render` into `shekyl-rpc-types`; a typed reason for `Held` beside a usable link (an exhausted epoch pin, or no address-hiding peer yet), in the shape IG-Q72 rules (core `INPUT_GRAMMAR.md`, draft #1162) | increment 3 (#1151), GS-C1 |
| GS-C3 | core | `GS-Q1`'s leg: the owning connection as an RPC listener kind, owner grant, host-only stop | — |
| GS-C4 | core | Correct `TSR-Q12`, `TOR_SUPERVISOR.md` §11 and core's FOLLOWUPS row for the RW naming: no leg carries Status to the GUI yet | — |
| GS-C5 | core | The chain-wide staking figures: total bonded principal (ESR-8) and the per-block staker inflow (`ARCHIVAL_BUDGET_SCHEDULE.md`), in `daemon_status`'s economics part beside `stake_share`. The GUI shows nothing for them until core's types carry them | the daemon's bond accounting |
| GS-1 | GUI | Installers from core's gitian archives, `tor/`, `tor-licenses/`, `tor-file`, on every platform (`GS-Q6`). Built: `release.yml` job `core-archives` with `scripts/release/core_archive.py` checks each archive's digest before unpacking (the Foundation-signed `SHA256SUMS` on a tag, GitHub's artifact digest for a dry run's gitian run); `DaemonManager` names the bundled Tor with `tor-file`; after bundling, the release reads Tor back out of every installer and checks it against core's pins and the bundled daemon's `--config-verify` (`src-tauri/binaries/README.md`) | The cut: `SHEKYL_CORE_REF` at the paired core tag, and for macOS that cut carries core #1310 (TSR-F18), without which the hardened runtime kills the signed `tor` at its first onion proof-of-work |
| GS-2 | GUI | Sidecar lifecycle: one target, child handle, the owning connection's stop, crash safety; the keep-running setting answers "not yet implemented"; the open wallet's address passed as `mine-address` when the GUI launches its node (IG-12: `mine-address` is set only at start, so switching wallets relaunches the node). Built: the node runs the GUI's network (`--network` at launch, mainnet by default, Rick 2026-11-13), a network switch relaunches it on that network's port from core's row and, with a configured data directory, in that network's own store; a switch waits while a wallet of another network is open | GS-C3 |
| GS-3 | GUI | Posture display, held review/verdict/pending, Settings → Network fixes, daemon config | GS-C1, GS-C2, GS-2 |
| GS-4 | GUI | Wire debt: RK-D1 adoption and the 4.0 reads (`daemon_status`, `block_header_show`, `curve_show`, `shard_coverage`, the `version` health probe) through `shekyl-rpc-client`, built; mining control on the owner leg remains | GS-2 for mining control |
| GS-5 | GUI | macOS installer Tor: carried by GS-1, by the same mechanism (`GS-Q6`) | As GS-1 |

## 6. Acceptance (real pathways, every platform with an installer)

Install the release-style installer, not a dev build. Then, each observed
in the GUI and in the daemon's log with the same words:
1. Fresh install → Tor bootstraps, the posture line clears, a send goes
   out over Tor and confirms on testnet.
2. Delete `tor/` → "Restarting (held)" with the `RestoreManaged` text; a
   send is reviewed as held and stays held; restore the bundle → it leaves
   and confirms.
3. Block outbound Tor with clearnet on → syncs, holds; the person takes
   `ClearnetOnly` → the send goes, labelled open-network.
4. Kill tor → Restarting, then Usable, same onion.
5. Quit the GUI → clean daemon stop within the bound; the next launch is
   seeded (seconds, not minutes). Kill the GUI → the daemon stops; no
   orphan holds the lock.
6. A transaction accepted while Tor is up but before any onion peer is
   handshaken is shown held until it leaves (GS-F3's falsifier).

## 7. Alpha resync

Until the full wallet adjustment lands, every cut resyncs the GUI with the
core it ships beside. It runs once per cut, not on a calendar, and retires
when that adjustment lands.

1. Early in the cut week, dispatch core's `gitian.yml` with
   `package_dry_run` on the head of `dev`, dispatched from `dev` with the
   commit as `tag`. Never dispatch it from a tag ref: the publish step keys
   on the ref, not the event, so a dispatch from a tag publishes.
2. Run this repo's release dry run (`release.yml`, `core_gitian_run`)
   against that run id. It compiles the GUI against the commit the run
   built and runs the live tests (`live_daemon_tests.rs`) against its
   daemon.
3. Fix whatever is red in one GUI resync PR for the cut, and sweep two
   things no build sees:
   - the grant-reach test's `AWAITING` list (`grant_reach_tests.rs`, the
     GUI's own calls and the engine's `DAEMON_METHODS`): is each entry
     still waiting on the increment it names?
   - chain facts written into GUI text (lock windows, block spacing,
     descriptions of mechanisms): each comes from core, never a copy.
4. The CLI's half is in core's tree. Its daemon calls are typed and its
   `--json` is the wire's reply, so it has no vocabulary of its own to
   resync; the sweep checks its human lines' words against core's
   `HEIGHT_SEMANTICS.md` §1.
5. The GUI is tagged beside core. Steps 1 and 2 run again at the freeze.

Caught without the resync: a wire name or shape change breaks the GUI's
build on the core PR that makes it (core's `gui-drift.yml`), and the GUI's
lock is checked against core `dev` daily (`core-drift.yml`).
