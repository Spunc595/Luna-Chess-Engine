# Luna's development history

Luna has been my spare-time hobby project since December 2025, built and rewritten across three languages and well over a hundred saved versions. This document lays out that history honestly, using file timestamps rather than memory or folder names (I wasn't consistent with version numbers, especially early on — see below). I've published a companion archive of the raw historical source files separately at [Luna-CE-History-Archive](https://github.com/Spunc595/Luna-CE-History-Archive), for anyone who wants to verify my authorship directly rather than take this document's word for it.

## December 2025 — JavaScript prototype

I wrote the very first version of Luna in JavaScript, December 11–18, 2025 (versions 0.1 through 2.3.1). A first attempt at a working chess engine, evolving over about a week.

## December 2025 – January 2026 — first Rust rewrite

Starting December 18, 2025, I rewrote Luna in Rust. This was an intensely iterative phase: I saved every small change as its own version, which is why well over a hundred version folders exist for barely three weeks of work (December 18 – January 12). The sequence reached a "Stabile" milestone on January 12, 2026.

I used both Gemini and DeepSeek as coding assistants during this period, sometimes writing a piece with one and having the other check it. I credited this openly at the time, in the most visible place I could: the engine's own UCI identity string printed `id author Daniele & Gemini` from this point through early 2026. I dropped DeepSeek first, once it started fabricating answers and doubling down when shown they were wrong. The `Gemini` credit itself stayed until around February 2026 (`Luna Chess Engine 4.1.0` onward just says `Daniele`) — not removed to hide anything, but because by then the code had been rewritten enough times that it reflected my own work far more than any of that early assisted output. The archive linked above shows this whole progression, unedited.

## January – April 2026 — continued Rust development

I continued developing in Rust, restarting version numbering from 1.0.1 on January 12, 2026 (the same day the previous phase reached "Stabile"), through to April 5, 2026. This period includes my first attempt at training a NNUE evaluation network from scratch (`Luna_AI_Lab`, starting January 27, 2026): a real PyTorch/Lightning training pipeline with checkpoints across training epochs. On the hardware I had available at the time (a personal laptop, no GPU), the resulting network was too weak to be usable in the engine.

## April – July 2026 — pause

I paused development for about three months. As a hobby I pursue in free time, my attention shifted elsewhere for a while.

## July 2026 — the Gemini incident

Around the time I resumed serious work, I used Google Gemini as a coding assistant to help clean up and translate parts of the codebase, then pushed the result to GitHub directly, without properly reviewing it first. This left visible traces: leftover AI-citation markers in code comments, conflicting version numbers across files, an orphaned unused UCI implementation, and a comment describing a network architecture ("HalfKA") that the code didn't actually implement. A forensic analysis by Christopher Whittington ([TalkChess thread](https://talkchess.com/viewtopic.php?t=86500)) identified this as unreviewed AI output rather than hand-written, hand-checked code.

I acknowledged this immediately and publicly on the same forum thread: *"I used an AI assistant (Gemini) as a co-pilot to help me implement my ideas and syntax."* I explained the mess came from hastily copy-pasting AI-generated cleanup and translation work straight to GitHub without review, and committed to cleaning it up — removing the dead code, fixing the warnings, and renaming the project "Luna CE". This was inexperience, not deception: I published AI-assisted work without reviewing it first, a real mistake I owned publicly at the time rather than hid.

## July – August 2026 — the current rewrite

That cleanup is reflected in the July 5–9, 2026 commits that open this repository's real history, followed by a full NNUE rewrite I started July 19, 2026 — a new accumulator/inference implementation supporting an architecture compatible with modern NNUE networks (768 inputs × 4 king buckets, mirrored, × 2 perspectives → 1024 hidden → 1 output).

**On the current network's origin**: the network currently embedded in the engine (`resources/net.bin`) is not self-trained. It's the trained network from [akimbo](https://github.com/jw1912/akimbo) by Jamie Whiting, used under its MIT license with full attribution (see the README's Acknowledgments and License sections). I set aside my own self-trained network from the January–February 2026 attempt as undertrained, for the reasons above. Using a permissively-licensed third-party network while writing my own search, move ordering, and accumulator code is common practice among hobbyist engines, and I'm disclosing it here and in the README rather than leaving it implicit.

This rewrite produced a series of dated snapshots through August 2026.

## Since — disciplined SPRT-driven testing

Since that rewrite, I've validated further changes statistically (SPRT, standard chess-engine testing methodology) before keeping them: pawn-based correction history, two rounds of SPSA parameter tuning, and singular extensions were all implemented, tested, and reverted after failing to show a real improvement. I implemented Lazy SMP multi-threading, tested it on real multi-core hardware, and kept it after it showed a genuine (if not formally SPRT-confirmed) gain. This discipline — being willing to revert work that doesn't hold up statistically — is ongoing practice for me, not a one-time effort.
