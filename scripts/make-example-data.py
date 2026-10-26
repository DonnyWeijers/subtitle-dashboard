#!/usr/bin/env python3
"""Writes a fake but realistic subtrans data folder to example/ (costs.jsonl + state.json), for trying the
dashboard locally. Made-up shows, nothing from a real library. Stdlib only.

The data covers the last DAYS days up to today, so "this month" has data: run it again to move the dates.
Same seed, same shape: the numbers follow what subtrans does (write-ups 48-51): 100-line batches, step 1 first,
~14% failing a free check and repaired (mostly by redoing batches with step 1), an AI judge on ~20%.

    python3 scripts/make-example-data.py [--days 75] [--out example]
"""
import argparse
import datetime as dt
import json
import random
from pathlib import Path

STEP1 = "openai/gpt-6-luna"
STEP2 = "google/gemini-3.7-flash"
JUDGE = "anthropic/claude-sonnet-5.5"
BATCH = 100
CHECKS = ["missing", "untranslated", "odd_length", "shifted"]

SERIES = {
    "Harbour Watch": (3, 12, ["Low Tide", "Fog", "Pilot", "Ballast", "The Lock", "Night Ferry", "Dry Dock", "Mooring"]),
    "The Night Bakery": (9, 10, ["Bread Week", "Pastry", "Semi Final", "The Final", "Biscuits", "Cake Week"]),
    "Northern Signal": (2, 8, ["Static", "Relay", "Blackout", "Frequency", "Dead Air"]),
    "Kitchen Brigade": (23, 14, ["Restaurant Wars", "Finale", "Street Food", "Quickfire", "Family Meal"]),
    "Ward 7": (12, 22, ["Triage", "Night Shift", "Code Blue", "Discharge", "Rounds"]),
    "Precinct 14": (14, 22, ["Stakeout", "Cold Case", "Badge", "Lineup", "Wiretap"]),
}
FILMS = ["Paper Lanterns (2016)", "The Long Crossing (2021)", "Glass Orchard (2019)", "Iron Meadow (2023)"]
NOTES = [
    ("meaning", "An idiom is translated word for word; the Dutch reads oddly."),
    ("meaning", "A joke depends on English wordplay and is lost."),
    ("fluency", "Stiff sentence; a native speaker would say it shorter."),
    ("grammar", "Wrong word order in a subordinate clause."),
    ("untranslated", "A short English line was left as is."),
    ("register", "Too formal ('u') for friends talking."),
]


def usd_step1(lines):
    return round(lines * random.uniform(9e-6, 1.6e-5), 7)


def usd_step2(lines):
    return round(lines * random.uniform(1.2e-4, 1.9e-4), 7)


def check_shares(fail=()):
    shares = {"missing": 0.0, "untranslated": round(random.uniform(0, 0.02), 4),
              "odd_length": round(random.uniform(0, 0.025), 4), "shifted": round(random.uniform(0, 0.012), 4)}
    limits = {"missing": 0.02, "untranslated": 0.03, "odd_length": 0.03, "shifted": 0.015}
    for name in fail:
        shares[name] = round(limits[name] * random.uniform(1.3, 4), 4)
    return shares


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--days", type=int, default=75)
    ap.add_argument("--out", default="example")
    args = ap.parse_args()
    random.seed(48)

    now = dt.datetime.now().replace(microsecond=0)
    start = now - dt.timedelta(days=args.days)
    calls, state = [], {}

    def log(t, kind, model, usd, title):
        calls.append({"t": t.isoformat(), "kind": kind, "model": model, "usd": usd, "title": title})

    # Which subtitles to translate: a backlog early on, then a few a day.
    items = []
    for show, (season, count, names) in SERIES.items():
        for ep in range(1, count + 1):
            items.append(("series", show, season, ep, names[(ep - 1) % len(names)]))
    for film in FILMS:
        items.append(("movie", film, None, None, None))
    random.shuffle(items)

    for i, (kind, show, season, ep, name) in enumerate(items):
        # first third in the first days (catching up), the rest spread out
        day = random.uniform(0, 4) if i < len(items) // 3 else random.uniform(4, args.days)
        t = start + dt.timedelta(days=day, hours=random.uniform(0, 14))
        if t > now:
            t = now - dt.timedelta(minutes=random.randint(5, 600))
        t = t.replace(microsecond=0)  # subtrans logs whole seconds
        if kind == "series":
            label = f"{show} - S{season:02d}E{ep:02d} - {name}"
            file = f"{label} WEBDL-1080p.en.srt"
            path = f"/data/series/{show}/Season {season}/{label} WEBDL-1080p.nl.srt"
            lines = random.randint(450, 1100)
        else:
            label = show.rsplit(" (", 1)[0]
            file = f"{show} WEBDL-1080p.en.hi.srt"
            path = f"/data/movies/{show}/{show} WEBDL-1080p.nl.srt"
            lines = random.randint(1100, 1800)
        title = f"{label} · {file}"
        batches = -(-lines // BATCH)

        usd, tier, models, step1 = 0.0, 1, {STEP1}, None
        for b in range(batches):
            cost = usd_step1(min(BATCH, lines - b * BATCH))
            usd += cost
            log(t + dt.timedelta(seconds=4 * b), "translate", STEP1, cost, title)
        failed, final_fail = [], []
        if random.random() < 0.14:  # step 1 fails a free check
            failed = random.choices(CHECKS, weights=[1, 1, 1, 8])[:1]
            step1 = {"model": STEP1, "checks": check_shares(failed), "failed": failed, "redo": []}
            route = random.choices(["step1", "step2", "file", "unfixed"], weights=[68, 12, 10, 10])[0]
            bad = random.randint(1, 2)
            passes = [(STEP1, "medium", bad)]
            if route != "step1":
                passes.append((STEP2, None, bad))
            if route in ("file", "unfixed"):
                passes.append((STEP2, None, "all"))
            for n, (model, effort, count) in enumerate(passes):
                last = n == len(passes) - 1
                still = failed if not last or route == "unfixed" else []
                for b in range(batches if count == "all" else count):
                    cost = (usd_step2 if model == STEP2 else usd_step1)(BATCH)
                    usd += cost
                    log(t + dt.timedelta(seconds=60 + 20 * n + b), "translate", model, cost, title)
                step1["redo"].append({"model": model, "effort": effort, "batches": count, "failed": still})
                models.add(model)
                if model == STEP2:
                    tier = 2
            final_fail = failed if route == "unfixed" else []

        score = accuracy = fluency = None
        problems, judge_usd = [], 0.0
        if random.random() < 0.2:
            score = max(40, min(97, round(random.gauss(82 if not final_fail else 62, 6))))
            accuracy = max(40, min(99, score + random.randint(-4, 4)))
            fluency = max(40, min(99, score + random.randint(-5, 3)))
            problems = [{"line": random.randint(1, lines), "type": k, "note": note}
                        for k, note in random.sample(NOTES, k=random.choice([0, 1, 2, 3, 5]))]
            judge_usd = round(random.uniform(0.008, 0.016), 5)
            log(t + dt.timedelta(seconds=150), "judge", JUDGE, judge_usd, title)

        at = (t + dt.timedelta(seconds=160)).isoformat()
        state[path] = {
            "title": title, "tier": tier, "model": STEP2 if tier == 2 else STEP1, "models_used": sorted(models),
            "lines": lines, "missing": random.randint(3, 20) if "missing" in final_fail else 0,
            "checks": check_shares(final_fail), "failed_checks": final_fail, "step1": step1,
            "score": score, "accuracy": accuracy, "fluency": fluency, "problems": problems[:5],
            "weak": bool(final_fail), "usd": round(usd, 5), "judge_usd": judge_usd,
            "seconds": round(20 + lines / 25 + (90 if step1 else 0)), "at": at,
            "history": [{"tier": tier, "score": score, "failed": final_fail, "at": at}],
        }

    # One test run, like subtrans' own TEST entries.
    log(start + dt.timedelta(hours=2), "translate", STEP1, 0.0004, "TEST sample S01E01")
    calls.sort(key=lambda c: c["t"])

    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    with open(out / "costs.jsonl", "w") as f:
        f.writelines(json.dumps(c) + "\n" for c in calls)
    with open(out / "state.json", "w") as f:
        json.dump(state, f, indent=1)
    print(f"{out}: {len(calls)} calls, {len(state)} translations, {start:%Y-%m-%d} to {now:%Y-%m-%d}")


if __name__ == "__main__":
    main()
