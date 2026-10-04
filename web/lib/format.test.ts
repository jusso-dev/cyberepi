import { describe, expect, it } from "vitest";
import { formatHours, reproductionPhase } from "./format";

describe("laboratory formatting", () => {
  it("marks the reproduction threshold", () => {
    expect(reproductionPhase(1.2)).toBe("expanding");
    expect(reproductionPhase(0.82)).toBe("contracting");
  });

  it("keeps multi-hour containment readable", () => {
    expect(formatHours(29 * 3600 + 14 * 60)).toBe("29h 14m");
    expect(formatHours(7 * 86400)).toBe("7 days");
  });
});
