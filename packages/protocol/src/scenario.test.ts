import { describe, it, expect } from "vitest";
import { delegateTo, effectiveCapability } from "./access";
import { delegableFields, NEVER_DELEGABLE } from "./requests";
import {
  cfoCapability,
  FINANCE_PRIVATE,
  humans,
  ownedAgents,
  principal,
  REGISTRY,
  registryCapabilities,
  registryCapability,
  resourceOwnerOf,
} from "./scenario";

describe("control-plane registry (company directory, 03 §6.1)", () => {
  it("every agent is owned by a human that exists in the registry", () => {
    const agents = REGISTRY.filter((p) => p.kind === "agent");
    expect(agents.length).toBeGreaterThan(0);
    for (const a of agents) {
      if (a.kind !== "agent") continue;
      const owner = principal(a.owner);
      expect(owner, `owner of ${a.id}`).toBeDefined();
      expect(owner!.kind).toBe("human");
    }
  });

  it("every human owns at least one agent and groups cleanly", () => {
    for (const h of humans()) {
      const owned = ownedAgents(h.id);
      expect(owned.length, `${h.id} owns agents`).toBeGreaterThan(0);
      expect(owned.every((a) => a.kind === "agent" && a.owner === h.id)).toBe(true);
    }
  });

  it("seeds an initial capability for every registry principal", () => {
    const caps = registryCapabilities();
    for (const p of REGISTRY) {
      expect(effectiveCapability(caps[p.id]), `cap for ${p.id}`).not.toBeNull();
      expect(effectiveCapability(registryCapability(p.id))).not.toBeNull();
    }
  });
});

describe("delegableFields — salary is structurally never offered (03 §6.2)", () => {
  it("drops NEVER_DELEGABLE fields from the CFO's own (salary-bearing) token", () => {
    const offered = delegableFields(cfoCapability());
    for (const banned of NEVER_DELEGABLE) expect(offered).not.toContain(banned);
    // …but still offers the rest of the finance fields.
    expect(offered).toEqual(expect.arrayContaining(["credits", "discount_tier", "gross"]));
  });

  it("a delegation built only from offered fields stays a subset and never carries salary", () => {
    const owner = cfoCapability();
    const offered = delegableFields(owner);
    const agent = delegateTo(owner, "agent:cfo/1", {
      by: "cfo",
      allowFields: offered,
      denyFields: [...NEVER_DELEGABLE],
    });
    const o = effectiveCapability(owner)!;
    const a = effectiveCapability(agent)!;
    expect(a.fields.has("employee_salary")).toBe(false);
    expect([...a.fields].every((f) => o.fields.has(f))).toBe(true); // caps(agent) ⊆ caps(owner)
  });
});

describe("resourceOwnerOf — request routing target (03 §6.3)", () => {
  it("routes finance_private requests to the CFO root owner", () => {
    expect(resourceOwnerOf(FINANCE_PRIVATE)).toBe("cfo");
  });
});
