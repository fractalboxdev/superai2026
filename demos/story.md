---
base_url: https://demo.contextful.work
viewport: 1280x720
slow_mo: 120
step_delay: 400
---

# Contextful product demo

Edit this file to change the demo. Run it with `/product-demo`.

Each `## Scene:` heading is a chapter. A `>` blockquote becomes the caption
overlaid on the video for that scene. Steps are `- action: argument` list items:

| action | argument | example |
| --- | --- | --- |
| `goto` | path or URL | `- goto: /directory` |
| `click` | Playwright selector | `- click: text=Delegate` |
| `fill` | `selector => value` (instant) | `- fill: input[name=q] => finops` |
| `type` | `selector => value` (typed out) | `- type: textarea => Hello` |
| `press` | keyboard key | `- press: Enter` |
| `write` | text typed into the focused element | `- write: Hello from the agent` |
| `hover` | selector | `- hover: nav a` |
| `wait for` | selector to appear | `- wait for: h1` |
| `expect` | selector must be visible | `- expect: text=Inbox` |
| `pause` | duration | `- pause: 2s` |
| `scroll` | selector, `top`, or `bottom` | `- scroll: bottom` |
| `caption` | replace caption (or `off`) | `- caption: New text` |

Prefix any action with `peer ` (e.g. `- peer goto: /`, `- peer write: …`) to run it
in a second off-camera tab in the same browser — the "agent peer". Same origin
means its edits reach the recorded tab live over cross-tab CRDT sync, so the
video shows them appearing in real time. The peer tab's own recording is discarded.

## Scene: Warm-up

- goto: /
- wait for: .weaver-surface p
- wait for: text=1 peer
- wait for: text=Dinesh's agent ·

## Scene: Every doc is live

> Contextful — every document is a live CRDT room. Edits sync between peers in real time.

- pause: 2s

## Scene: An agent peer is in the room

> Dinesh's agent is already here — a live peer with its own presence, not a mock.

- expect: text=1 peer
- pause: 2s

## Scene: The agent edits in real time

> Watch it work: every keystroke is a CRDT op, landing in this view live.

- scroll: text=Dinesh's agent ·
- pause: 9s

## Scene: Monica (CFO) asks the question

> The demo question comes from Monica (CFO) — one click, and she tags her analyst agent right in the doc, over the on-prem relay.

- goto: /?sync=ws://127.0.0.1:7878
- wait for: text=▶ Demo Q by CFO
- pause: 1s
- click: text=▶ Demo Q by CFO
- expect: text=Typing as CFO…
- wait for: text=unit economics of our compression product
- scroll: text=unit economics of our compression product
- pause: 1.5s

## Scene: Her agent answers from brain memory

> Her analyst agent is watching the doc. It checks her capability token and answers from the Markdown memory on this machine — live, over the relay.

- pause: 6s
- wait for: text=A (cfo · for cfo
- scroll: text=A (cfo · for cfo
- pause: 3s

## Scene: Now impersonate Dinesh (CTO)'s agent

> Same room, different badge — now acting as Dinesh (CTO)'s agent, whose token carries no finance grant.

- click: button.cf-actor:has-text("Dinesh (CTO)'s agent")
- pause: 1.5s

## Scene: The same finance question, denied

> He reaches for the same private finance view Monica just used — and policy denies it before any data moves.

- click: text=stripe / finance_private
- pause: 1s
- click: text=Run query as Dinesh (CTO)'s agent
- wait for: .cf-result--deny
- expect: text=⛔ Denied
- scroll: .cf-result--deny
- pause: 3s

## Scene: Connectors feed the brain

> Context comes from connectors — Stripe and Exa already sync into the brain on this machine.

- goto: /connectors
- wait for: h1
- expect: text=Connectors
- expect: text=connected
- hover: .cn-card
- pause: 2s

## Scene: Switch a source on

> Turning on a source is one toggle — it starts ingesting into local views, nothing leaves the machine.

- click: button[aria-label="GitHub — off"]
- expect: button[aria-label="GitHub — connected"]
- pause: 2s

## Scene: Company directory

> Access control starts in the directory: every person and agent, and what their capability token actually grants.

- goto: /directory
- wait for: h1
- expect: text=Company directory
- hover: .ac-caps
- pause: 1.5s
- scroll: bottom
- pause: 1.5s
- scroll: top

## Scene: Delegate to your agent

> Delegation is scoped. From the directory, hand a narrowed token to your own agent.

- click: .ac-agent__action
- wait for: h1
- expect: text=Delegate to my agent
- pause: 1.5s

## Scene: Narrow the scope

> Drop a field and the token shrinks — salary can never be delegated at all.

- click: .ac-form .cf-chip
- pause: 1s
- click: button.cf-block
- expect: text=narrowed, never widened
- pause: 2.5s

## Scene: The inbox

> Requests your agents make for someone else's data land in the owner's inbox, each with its exact scope.

- goto: /inbox
- wait for: h1
- expect: text=Inbox
- hover: .ac-request__scope
- pause: 2.5s

## Scene: Approve exactly the scope

> Approve mints a token for exactly the requested fields, rows, and TTL — nothing wider.

- click: text="Approve (scoped)"
- expect: text=✓ Approved
- pause: 2s

## Scene: A hard floor

> Some requests have no approve path at all — salary is forbidden outright, for any agent.

- scroll: .cf-forbidden
- expect: .cf-forbidden
- pause: 2.5s

## Scene: Every decision is audited

> Each grant and denial lands in the audit trail — who minted what, to whom, with which TTL.

- scroll: .ac-audit
- expect: .cf-log__row
- pause: 2.5s

## Scene: Back in the room

> The agent kept working the whole time — that's Contextful: local-first context, capability-scoped sharing.

- goto: /?sync=off
- wait for: text=Dinesh's agent ·
- pause: 4s
