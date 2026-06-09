/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
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
