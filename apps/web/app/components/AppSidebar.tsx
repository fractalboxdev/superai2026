import { Link, useLocation } from "react-router";
import { DOCS, DEFAULT_DOC_ID } from "@/lib/docs";
import DocDebugMenu from "@/components/DocDebugMenu";

/** Line icons in the Stripe sidebar idiom — 16px, stroke = currentColor. */
const ICONS = {
  documents: (
    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M9.5 1.5H4a1 1 0 0 0-1 1v11a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V5l-3.5-3.5Z" />
      <path d="M9.5 1.5V5H13" />
    </svg>
  ),
  directory: (
    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <circle cx="8" cy="5" r="2.5" />
      <path d="M2.5 14a5.5 5.5 0 0 1 11 0" />
    </svg>
  ),
  connectors: (
    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M5.5 2v3.5M10.5 2v3.5" />
      <path d="M3.5 5.5h9V8a4.5 4.5 0 0 1-9 0V5.5Z" />
      <path d="M8 12.5V15" />
    </svg>
  ),
  memory: (
    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <ellipse cx="8" cy="3.5" rx="5.5" ry="2" />
      <path d="M2.5 3.5v9c0 1.1 2.46 2 5.5 2s5.5-.9 5.5-2v-9" />
      <path d="M2.5 8c0 1.1 2.46 2 5.5 2s5.5-.9 5.5-2" />
    </svg>
  ),
  delegate: (
    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M2.5 13.5V10A3.5 3.5 0 0 1 6 6.5h7" />
      <path d="m10 3.5 3.5 3L10 9.5" />
    </svg>
  ),
  inbox: (
    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M2 9.5h3.5L7 11.5h2l1.5-2H14" />
      <path d="M3.5 2.5h9l1.5 7v4h-12v-4l1.5-7Z" />
    </svg>
  ),
} as const;

function Mark() {
  return (
    <svg viewBox="0 0 32 32" role="img" aria-label="Contextful">
      <defs>
        <linearGradient id="sbmark" x1="0" y1="0" x2="32" y2="32" gradientUnits="userSpaceOnUse">
          <stop offset="0" stopColor="#6366f1" />
          <stop offset="1" stopColor="#0ea5e9" />
        </linearGradient>
      </defs>
      <rect width="32" height="32" rx="8" fill="url(#sbmark)" />
      <path d="M22 11.4a7 7 0 1 0 0 9.2" fill="none" stroke="#fff" strokeWidth="3" strokeLinecap="round" />
      <circle cx="22.6" cy="16" r="2.6" fill="#f59e0b" />
    </svg>
  );
}

type NavItem = {
  href: string;
  label: string;
  icon: keyof typeof ICONS;
  /** Marks active for any pathname with this prefix (in addition to `href`). */
  also?: string;
};

const PRIMARY: NavItem[] = [
  { href: "/", label: "Documents", icon: "documents", also: "/docs" },
  { href: "/directory", label: "Directory", icon: "directory" },
  { href: "/connectors", label: "Connectors", icon: "connectors" },
  { href: "/memory", label: "Memory", icon: "memory" },
];

const ACCESS: NavItem[] = [
  { href: "/delegate", label: "Delegation", icon: "delegate" },
  { href: "/inbox", label: "Inbox", icon: "inbox" },
];

function isActive(item: NavItem, pathname: string): boolean {
  if (item.href === "/") return pathname === "/" || (!!item.also && pathname.startsWith(item.also));
  return pathname.startsWith(item.href);
}

function NavLink({ item, pathname, count }: { item: NavItem; pathname: string; count?: number }) {
  const active = isActive(item, pathname);
  return (
    <Link
      to={item.href}
      className={`sb__link${active ? " sb__link--on" : ""}`}
      aria-current={active ? "page" : undefined}
    >
      <span className="sb__icon">{ICONS[item.icon]}</span>
      {item.label}
      {count ? <span className="sb__count">{count}</span> : null}
    </Link>
  );
}

/**
 * Permanent app sidebar (Stripe idiom): workspace header, primary nav
 * (documents · directory · connectors · memory) with the document rooms
 * nested under Documents, and the access-control surfaces below.
 */
export function AppSidebar({ inboxCount }: { inboxCount?: number }) {
  const { pathname } = useLocation();
  const onDocs = pathname === "/" || pathname.startsWith("/docs");
  const activeDocId = pathname.startsWith("/docs/")
    ? decodeURIComponent(pathname.split("/")[2] ?? "")
    : DEFAULT_DOC_ID;

  return (
    <nav className="sb" aria-label="Primary">
      <Link to="/" className="sb__workspace">
        <span className="sb__mark">
          <Mark />
        </span>
        <span className="sb__id">
          <span className="sb__name">Contextful</span>
          <span className="sb__sub">Pied Piper · demo</span>
        </span>
      </Link>

      <ul className="sb__nav">
        {PRIMARY.map((item) => (
          <li key={item.href}>
            <NavLink item={item} pathname={pathname} />
            {item.icon === "documents" && (
              <ul className="sb__docs">
                {DOCS.map((d) => {
                  const current = onDocs && d.id === activeDocId;
                  return (
                    <li key={d.id} className="sb__docrow">
                      <Link
                        to={d.id === DEFAULT_DOC_ID ? "/" : `/docs/${d.id}`}
                        className={`sb__doc${current ? " sb__doc--on" : ""}`}
                        aria-current={current ? "page" : undefined}
                      >
                        {d.title}
                      </Link>
                      <DocDebugMenu docId={d.id} docTitle={d.title} />
                    </li>
                  );
                })}
              </ul>
            )}
          </li>
        ))}
      </ul>

      <p className="sb__section">Access control</p>
      <ul className="sb__nav">
        {ACCESS.map((item) => (
          <li key={item.href}>
            <NavLink
              item={item}
              pathname={pathname}
              count={item.href === "/inbox" ? inboxCount : undefined}
            />
          </li>
        ))}
      </ul>
    </nav>
  );
}
