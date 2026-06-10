---
name: pitch-review
description: Review a pitch, deck, demo script, or presentation narrative through a panel of three independent experts — an ex-founder/operator, an early-stage VC, and a communications/narrative expert. Use when the user wants critical, decision-grade feedback on a presentation or demo before building slides or presenting it. Accepts a file path or description of the artifact; defaults to the most recently created PRESENTATION.md / deck / demo script in the repo.
---

# Pitch Review — three-expert panel

Deliver a brutally honest, decision-grade review of a presentation/demo from three
**independent** expert lenses, then synthesize. The point of a panel is *non-blended*
critique: each persona must reach its own verdict before you reconcile them.

## When to use

The user asks for feedback / a review / a "gut check" on a pitch, deck, demo story,
demo script, narrative, or presentation. Also good before turning a story into slides.

## Inputs

- **Target artifact**: a file path (e.g. `PRESENTATION.md`), or a description. If none
  is given, find the most relevant presentation/demo/deck file in the repo (look for
  `PRESENTATION.md`, `*deck*`, `*pitch*`, `*demo*`, slides under `apps/landing`/`docs`).
- Read the artifact fully before reviewing. If a real demo/app exists, note that running
  it is the strongest signal — flag if the demo's believability can't be verified.

## How to run

1. **Read the target artifact in full.** Identify the core claim, the demo's "money
   shot," and the ask/CTA.
2. **Run the three lenses independently.** Prefer spawning one subagent per persona (in
   parallel) so critiques don't homogenize. Give each the artifact path and its rubric
   below; require the structured per-persona output. If running inline, write each
   persona's section to completion before starting the next — do not blend voices.
3. **Synthesize** across the three: cross-cutting themes, contradictions between
   experts, and a single **prioritized fix list** (P0/P1/P2).
4. **Write the review to `.tmp/presentation-review.md`** (per repo convention for
   long-form output), open it in Zed with Markdown preview, and in chat output ONLY a
   one-line pointer to the file path. Do not recap the review in chat.

## The panel

### 1. Ex-founder / operator (has pitched, raised, and shipped)
Cares about: Is the problem real and *painful enough to pay for*? Is "why now" honest?
**Will the demo actually work live, or is it fragile/staged?** Is the wedge sharp? Does
it survive "why hasn't an incumbent done this?" Founder–market fit and earned insight
(e.g. lived experience). Distribution / first 10 customers. Where is the hand-waving a
technical buyer will poke? Is this a vitamin or a painkiller?
Brutal lens: name the slide where a skeptical buyer checks out, and the demo step most
likely to break on stage.

### 2. Early-stage VC (seed/Series A investor)
Cares about: market size and whether this is a **company or a feature**; the
wedge→expansion path; moat/defensibility; competition (name real comparables and the
"do nothing / status quo" option); "what has to be true" for this to be huge; the
team's right-to-win; business model; the one-line memorable thesis. Would they take a
second meeting?
Brutal lens: state the strongest reason to pass, and the single question whose answer
decides the check.

### 3. Communications / narrative expert (storytelling & delivery)
Cares about: a single sticky throughline (rule of one); does the cold open hook in
<15s; pacing and where attention drops; jargon; whether devices (memes, themes,
analogies) *serve* the message or distract; one clear money shot in the demo; slide
economy; a crisp CTA; memorability of the logline. Offer a tighter rewrite of the
logline and the opening line if you can beat them.
Brutal lens: what to cut, and the one sentence the audience should remember a week
later.

## Required output per persona

- **Verdict** (one line) + **Score** /10 (and the bar they're scoring against)
- **Top strengths** (2–3, specific)
- **Brutal truths / biggest risks** (3–5, each tied to a section or line)
- **Specific fixes** (section/line-level, actionable — not "tighten the narrative")
- **The one question they'd ask the presenter**

## Synthesis section (you, after the panel)

- **Where the experts agree** (highest-confidence fixes)
- **Where they disagree** (and your call on the tradeoff)
- **Prioritized fix list**: P0 (do before slides), P1 (before presenting), P2 (nice)
- **Overall verdict**: ship / revise / rework, in one paragraph

## Style

Be specific and unsparing; cite sections. Flattery is useless here — the user wants the
review that prevents a bad pitch, not encouragement. Keep each fix concrete enough to
act on without a follow-up question.
