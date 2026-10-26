import { describe, expect, it } from "vitest";

import { check, dayRange, daysAgo, episode, euro, model, percent, repair } from "./format";

// Intl puts a narrow no-break space between € and the amount.
const plain = (s: string) => s.replace(/\s/g, " ");

describe("euro", () => {
  it("shows enough decimals for small amounts", () => {
    expect(plain(euro(2.5523))).toBe("€ 2,55");
    expect(plain(euro(0.118))).toBe("€ 0,118");
    expect(plain(euro(0.0084366))).toBe("€ 0,0084");
    expect(plain(euro(0))).toBe("€ 0,00");
    expect(euro(null)).toBe("–");
  });
});

describe("labels", () => {
  it("formats episodes, models, checks and repairs", () => {
    expect(episode({ season: 28, episode: 3 })).toBe("S28E03");
    expect(episode({ season: null, episode: null })).toBe("");
    expect(model("openai/gpt-6-luna")).toBe("gpt-6-luna");
    expect(check("odd_length")).toBe("Odd line length");
    expect(check("new_check")).toBe("new check");
    expect(repair("step2-file")).toBe("Whole file, step 2");
    expect(percent(11.4)).toBe("11,4%");
  });
});

describe("days", () => {
  it("lists every day across a month end", () => {
    expect(dayRange("2026-09-29", "2026-10-02")).toEqual(["2026-09-29", "2026-09-30", "2026-10-01", "2026-10-02"]);
    expect(dayRange("2026-10-02", "2026-10-01")).toEqual([]);
    expect(daysAgo(3, new Date(2026, 9, 2, 12))).toBe("2026-09-29");
  });
});
