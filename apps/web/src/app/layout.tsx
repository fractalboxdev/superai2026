import type { Metadata, Viewport } from "next";
import "@superai2026/design-system/styles.css";
import "./globals.css";
import { HyperDXInit } from "@/components/hyperdx-init";

// Per-page metadata via the Next.js Metadata API (parent CLAUDE.md SEO rules).
export const metadata: Metadata = {
  metadataBase: new URL("https://demo.contextful.work"),
  applicationName: "Contextful",
  title: {
    default: "Contextful",
    template: "%s · Contextful",
  },
  description:
    "Local-first company brain. Every agent sees only the context it's permitted to — scoped by capability.",
  openGraph: {
    title: "Contextful",
    description:
      "Local-first company brain. Every agent sees only the context it's permitted to — scoped by capability.",
    type: "website",
  },
  twitter: { card: "summary_large_image" },
};

export const viewport: Viewport = {
  themeColor: [
    { media: "(prefers-color-scheme: light)", color: "#f8fafc" },
    { media: "(prefers-color-scheme: dark)", color: "#020617" },
  ],
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <head>
        <link rel="preconnect" href="https://fonts.googleapis.com" />
        <link
          rel="preconnect"
          href="https://fonts.gstatic.com"
          crossOrigin="anonymous"
        />
        <link
          href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700;800&display=swap"
          rel="stylesheet"
        />
      </head>
      <body>
        <HyperDXInit />
        {children}
      </body>
    </html>
  );
}
