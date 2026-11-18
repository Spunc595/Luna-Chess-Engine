#!/usr/bin/env python3
"""Exact SPRT trinomial LLR, reimplementing cutechess-cli 1.5.1's
projects/lib/src/sprt.cpp (read on luna-server, 2026-11-18) bit for bit:
Dirichlet(0.5,0.5,0.5) regularization, BayesElo/drawElo estimated out of
sample, LLR = wins*ln(p1win/p0win) + losses*ln(p1loss/p0loss) +
draws*ln(p1draw/p0draw), bounds ln(beta/(1-alpha)) / ln((1-beta)/alpha).

Used to compute the CUMULATIVE LLR across all Esperimento 06 blocks from
pooled (wins, losses, draws) of engine "nuova" vs "rif" -- cutechess's own
-sprt is per-process and would reset at every block boundary, so it is NOT
used as the real stopping rule; this module is.

    python3 exp06_llr.py <wins> <losses> <draws> [elo0] [elo1] [alpha] [beta]
"""
import math
import sys


def bayeselo_from_probs(p_win, p_loss):
    bayes_elo = 200.0 * math.log10(p_win / p_loss * (1.0 - p_loss) / (1.0 - p_win))
    draw_elo = 200.0 * math.log10((1.0 - p_loss) / p_loss * (1.0 - p_win) / p_win)
    return bayes_elo, draw_elo


def scale_from_draw_elo(draw_elo):
    x = 10.0 ** (-draw_elo / 400.0)
    return 4.0 * x / ((1.0 + x) ** 2)


def probs_from_bayeselo(bayes_elo, draw_elo):
    p_win = 1.0 / (1.0 + 10.0 ** ((draw_elo - bayes_elo) / 400.0))
    p_loss = 1.0 / (1.0 + 10.0 ** ((draw_elo + bayes_elo) / 400.0))
    p_draw = 1.0 - p_win - p_loss
    return p_win, p_loss, p_draw


def sprt_status(wins, losses, draws, elo0, elo1, alpha, beta):
    """Mirrors Sprt::status() in sprt.cpp exactly, including the
    half-point-per-class Jeffreys regularization."""
    if wins < 0 or losses < 0 or draws < 0:
        raise ValueError("negative count")
    w = wins + 0.5
    l = losses + 0.5
    d = draws + 0.5
    n = w + l + d
    p_win, p_loss = w / n, l / n

    bayes_elo, draw_elo = bayeselo_from_probs(p_win, p_loss)
    s = scale_from_draw_elo(draw_elo)

    p0win, p0loss, p0draw = probs_from_bayeselo(elo0 / s, draw_elo)
    p1win, p1loss, p1draw = probs_from_bayeselo(elo1 / s, draw_elo)

    llr = (w * math.log(p1win / p0win) +
           l * math.log(p1loss / p0loss) +
           d * math.log(p1draw / p0draw))

    l_bound = math.log(beta / (1.0 - alpha))
    u_bound = math.log((1.0 - beta) / alpha)

    if llr >= u_bound:
        result = "AcceptH1"
    elif llr <= l_bound:
        result = "AcceptH0"
    else:
        result = "Continue"

    return {
        "llr": llr, "lBound": l_bound, "uBound": u_bound, "result": result,
        "bayesElo": bayes_elo, "drawElo": draw_elo, "scale": s,
    }


def main():
    wins, losses, draws = (float(x) for x in sys.argv[1:4])
    elo0 = float(sys.argv[4]) if len(sys.argv) > 4 else 0.0
    elo1 = float(sys.argv[5]) if len(sys.argv) > 5 else 5.0
    alpha = float(sys.argv[6]) if len(sys.argv) > 6 else 0.05
    beta = float(sys.argv[7]) if len(sys.argv) > 7 else 0.05
    st = sprt_status(wins, losses, draws, elo0, elo1, alpha, beta)
    n = wins + losses + draws
    print(f"n={n:.0f} W={wins:.0f} L={losses:.0f} D={draws:.0f}")
    print(f"llr={st['llr']:.4f}  lBound={st['lBound']:.4f}  uBound={st['uBound']:.4f}  -> {st['result']}")
    print(f"bayesElo={st['bayesElo']:.3f}  drawElo={st['drawElo']:.3f}  scale={st['scale']:.4f}")


if __name__ == "__main__":
    main()
