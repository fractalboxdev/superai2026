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

## Scene: Every doc is live

> Contextful — every document is a live CRDT room. Edits sync between peers in real time.

- goto: /
- wait for: .weaver-surface p
- pause: 2s

## Scene: An agent peer joins

> Dinesh's agent opens the same doc from its own session — presence lights up the moment it joins.

- peer goto: /
- peer wait for: .weaver-surface p
- peer click: button.cf-actor:has-text("Dinesh")
- wait for: text=cross-tab
- pause: 2s

## Scene: The agent edits in real time

> Now watch it write — every keystroke is a CRDT op, landing in this view live.

- peer click: .weaver-surface p:last-of-type
- peer press: Meta+ArrowDown
- peer press: Enter
- peer write: Dinesh's agent: pulled this week's numbers — net spend is 12% under budget once credits land.
- expect: text=12% under budget
- peer press: Enter
- peer write: Flagging the discount tier for Monica to confirm.
- expect: text=Monica to confirm
- pause: 2.5s

## Scene: Company directory

> The directory shows every person and agent in the org — the principals that capabilities are granted to.

- goto: /directory
- wait for: h1
- expect: text=Company directory
- scroll: bottom
- pause: 1.5s
- scroll: top

## Scene: Delegate to your agent

> Delegation is scoped: pick a view, pick the fields, and your agent can see exactly that — nothing more.

- goto: /delegate
- wait for: h1
- pause: 2.5s

## Scene: The inbox

> Incoming grant requests land in the inbox, where a human approves or rejects each scope.

- goto: /inbox
- wait for: h1
- expect: text=Inbox
- pause: 2.5s

## Scene: Back in the room

> The agent's edits are already in the doc — that's Contextful: local-first context, capability-scoped sharing.

- goto: /
- wait for: text=Monica to confirm
- pause: 3s
