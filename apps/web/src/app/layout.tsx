import type { Metadata } from "next";
import "./globals.css";

// Per-page metadata via the Next.js Metadata API (parent CLAUDE.md SEO rules).
// TODO: set metadataBase to the production domain.
export const metadata: Metadata = {
  metadataBase: new URL("https://example.com"),
  title: {
    default: "superai2026",
    template: "%s · superai2026",
  },
  description: "Placeholder web app for superai2026.",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
