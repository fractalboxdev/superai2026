import { Link, useLocation } from "react-router";

const LINKS = [
  { href: "/", label: "Console" },
  { href: "/directory", label: "Directory" },
  { href: "/delegate", label: "Delegation" },
  { href: "/inbox", label: "Inbox" },
];

/** Cross-surface nav for the web access-control UI (specs/03 §6). */
export function AppNav({ inboxCount }: { inboxCount?: number }) {
  const { pathname } = useLocation();
  return (
    <nav className="ac-nav" aria-label="Access control">
      <Link to="/" className="ac-nav__brand">
        Contextful
      </Link>
      <ul className="ac-nav__links">
        {LINKS.map((l) => {
          const active = l.href === "/" ? pathname === "/" : pathname.startsWith(l.href);
          return (
            <li key={l.href}>
              <Link
                to={l.href}
                className={`ac-nav__link${active ? " ac-nav__link--on" : ""}`}
                aria-current={active ? "page" : undefined}
              >
                {l.label}
                {l.href === "/inbox" && inboxCount ? (
                  <span className="ac-nav__count">{inboxCount}</span>
                ) : null}
              </Link>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
