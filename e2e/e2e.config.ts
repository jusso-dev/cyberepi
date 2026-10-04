import type { E2EConfig } from "e2e";
import { web } from "@e2e-dev/web";

const url = process.env.CYBEREPI_URL ?? "http://cyberepi.192.168.1.19.sslip.io";

export default {
  timeout: 180_000,
  targets: [
    {
      name: "chromium",
      engine: web({ viewport: { width: 1440, height: 900 } }),
      app: { url },
    },
  ],
} satisfies E2EConfig;
