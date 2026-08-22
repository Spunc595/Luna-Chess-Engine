# Luna's development history

Luna has been a spare-time hobby project since December 2025, built and rewritten across three languages and well over a hundred saved versions. This document lays out that history honestly, using file timestamps rather than memory or folder names (version numbers were not always applied consistently, especially early on — see below). A companion archive of the raw historical source files is published separately at [Luna-History-Archive](https://github.com/Spunc595/Luna-History-Archive) for anyone who wants to verify authorship directly rather than take this document's word for it.

## December 2025 — JavaScript prototype

The very first version of Luna was written in JavaScript, December 11–18, 2025 (versions 0.1 through 2.3.1). A first attempt at a working chess engine, evolving over about a week.

## December 2025 – January 2026 — first Rust rewrite

Starting December 18, 2025, Luna was rewritten in Rust. This was an intensely iterative phase: every small change was saved as its own version, which is why well over a hundred version folders exist for barely three weeks of work (December 18 – January 12). The sequence reached a "Stabile" milestone on January 12, 2026.

## January – April 2026 — continued Rust development

Development continued in Rust, with version numbering restarted from 1.0.1 on January 12, 2026 (the same day the previous phase reached "Stabile"), through to April 5, 2026. This period includes the first attempt at training a NNUE evaluation network from scratch (`Luna_AI_Lab`, starting January 27, 2026): a real PyTorch/Lightning training pipeline with checkpoints across training epochs. On the hardware available at the time (a personal laptop, no GPU), the resulting network was too weak to be usable in the engine.

## April – July 2026 — pause

Development paused for about three months. As a hobby pursued in free time, attention shifted elsewhere for a while.

## July 2026 — the Gemini incident

Around the time serious work resumed, Daniele used Google Gemini as a coding assistant to help clean up and translate parts of the codebase, then pushed the result to GitHub directly, without properly reviewing it first. This left visible traces: leftover AI-citation markers in code comments, conflicting version numbers across files, an orphaned unused UCI implementation, and a comment describing a network architecture ("HalfKA") that the code didn't actually implement. A forensic analysis by Christopher Whittington ([TalkChess thread](https://talkchess.com/viewtopic.php?t=86500)) identified this as unreviewed AI output rather than hand-written, hand-checked code.

Daniele acknowledged this immediately and publicly on the same forum thread: *"I used an AI assistant (Gemini) as a co-pilot to help me implement my ideas and syntax."* He explained the mess came from hastily copy-pasting AI-generated cleanup and translation work straight to GitHub without review, and committed to cleaning it up — removing the dead code, fixing the warnings, and renaming the project "Luna CE". This is inexperience, not deception: publishing AI-assisted work without reviewing it first was a real mistake, owned publicly at the time rather than hidden.

## July – August 2026 — the current rewrite

That cleanup is reflected in the July 5–9, 2026 commits that open this repository's real history, followed by a full NNUE rewrite starting July 19, 2026 — a new accumulator/inference implementation supporting an architecture compatible with modern NNUE networks (768 inputs × 4 king buckets, mirrored, × 2 perspectives → 1024 hidden → 1 output).

**On the current network's origin**: the network currently embedded in the engine (`resources/net.bin`) is not self-trained. It is the trained network from [akimbo](https://github.com/jw1912/akimbo) by Jamie Whiting, used under its MIT license with full attribution (see the README's Acknowledgments and License sections). Luna's own self-trained network from the January–February 2026 attempt was set aside as undertrained for the reasons above. Using a permissively-licensed third-party network while writing one's own search, move ordering, and accumulator code is common practice among hobbyist engines, and is disclosed here and in the README rather than left implicit.

This rewrite produced a series of dated snapshots through August 2026.

## Since — disciplined SPRT-driven testing

Since that rewrite, further changes have been validated statistically (SPRT, standard chess-engine testing methodology) before being kept: pawn-based correction history, two rounds of SPSA parameter tuning, and singular extensions were all implemented, tested, and reverted after failing to show a real improvement. Lazy SMP multi-threading was implemented, tested on real multi-core hardware, and kept after showing a genuine (if not formally SPRT-confirmed) gain. This discipline — willingness to revert work that doesn't hold up statistically — is ongoing project practice, not a one-time effort.
