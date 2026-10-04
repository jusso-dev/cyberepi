import type { NextConfig } from "next";

const controller = process.env.CONTROLLER_URL ?? "http://127.0.0.1:8080";

const nextConfig: NextConfig = {
  output: "standalone",
  async rewrites() {
    return [
      { source: "/api/:path*", destination: `${controller}/api/:path*` },
      { source: "/healthz", destination: `${controller}/healthz` },
    ];
  },
};

export default nextConfig;
