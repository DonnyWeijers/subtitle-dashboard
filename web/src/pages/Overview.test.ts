import { describe, expect, it } from "vitest";

import { costChartData } from "./Overview";

describe("costChartData", () => {
  it("stacks models per day, most expensive first, with empty days", () => {
    const { models, data } = costChartData(
      [
        { key: "2026-10-01", model: "openai/gpt-6-luna", eur: 0.01, calls: 3 },
        { key: "2026-10-01", model: "google/gemini-3.7-flash", eur: 0.1, calls: 1 },
        { key: "2026-10-03", model: "openai/gpt-6-luna", eur: 0.02, calls: 4 },
      ],
      "2026-10-01",
      "2026-10-03",
    );
    expect(models).toEqual(["google/gemini-3.7-flash", "openai/gpt-6-luna"]);
    expect(data).toEqual([
      { day: "2026-10-01", "openai/gpt-6-luna": 0.01, "google/gemini-3.7-flash": 0.1 },
      { day: "2026-10-02" },
      { day: "2026-10-03", "openai/gpt-6-luna": 0.02 },
    ]);
  });
});
