/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  // The protocol package ships raw TypeScript; let Next transpile it.
  transpilePackages: ["@superai2026/protocol"],
  // Allow the Tailscale tailnet host so the dev server is reachable over MagicDNS.
  experimental: {
    allowedDevOrigins: ["https://*.ts.net"],
  },
};

export default nextConfig;
