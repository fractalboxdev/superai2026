/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  // The protocol package ships raw TypeScript; let Next transpile it.
  transpilePackages: ["@superai2026/protocol"],
  // Allow the Tailscale tailnet host so the dev server is reachable over MagicDNS.
  experimental: {
    allowedDevOrigins: ["https://*.ts.net"],
  },
  // Keep the HyperDX/OpenTelemetry SDK out of the server bundle so its native
  // auto-instrumentation (require-in-the-middle patching) works at runtime.
  serverExternalPackages: ["@hyperdx/node-opentelemetry"],
  webpack: (config, { isServer }) => {
    if (isServer) {
      // OpenTelemetry pulls in optional deps it warns about when bundling.
      config.ignoreWarnings = [{ module: /opentelemetry/ }];
    }
    return config;
  },
};

export default nextConfig;
