# superai2026 product demo

Story script for the FlareDispatch `product-demo` run. Each `## ` heading is one
story; the prose below it is handed to the demo-agent, which drives the deployed
site over CDP and records one continuous rrweb replay with a chapter per story.

Keep prose imperative and observable ("click X, confirm Y appears") — the agent
turns it into CDP actions and the run asserts on what it sees. Story names must
be unique; they become the replay's chapter markers.

## landing-hero
Open the landing site at the root URL. Wait for the hero section to render.
Confirm the main `<h1>` headline and the primary call-to-action button are
visible above the fold. Capture the hero as the story's key screenshot.

## landing-scroll
From the hero, scroll down through the landing page. Confirm each major section
heading comes into view in order and that no section renders empty or broken.
Stop at the footer and confirm the footer navigation links are present.

## open-web-app
Navigate to the web app (follow the primary "Get started" / "Launch app" CTA, or
go to the `/app` path if the CTA is absent). Confirm the app shell loads — a
visible header and the main content region — without a blank screen or an error
overlay. Capture the loaded app as the story's key screenshot.

## capability-console-flow-a
In the web app, find the "Acting as" switcher and the "Run a flow" controls in
the left sidebar. Click "Flow A · request → approve". Confirm a permission
request card appears in the right panel showing the CTO's agent asking for
`credits` and `discount_tier` on `stripe/finance_private` with `employee_salary`
denied. Click "Approve (scoped)". Confirm the query result then shows a
net-of-credits answer and that the audit trail logs a minted scoped token.
Capture the approved result as the story's key screenshot.

## capability-console-flow-b
Click "Flow B · salary invariant". Confirm the permission request card shows the
Engineering agent reaching for `employee_salary`, that it is marked **forbidden /
no approval path** (no Approve button is offered), and that the audit trail logs
the request as blocked. Confirm the document still shows the salary column as
redacted. This is the provable "the CTO's agent can't read the CEO's salary"
claim, observed in the UI.
