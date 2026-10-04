import type { Metadata } from "next";
import "@fontsource/fraunces/500.css";
import "@fontsource/ibm-plex-sans/400.css";
import "@fontsource/ibm-plex-sans/500.css";
import "@fontsource/ibm-plex-mono/400.css";
import "./globals.css";
import { Shell } from "@/components/shell";

export const metadata: Metadata = {
  title: "CyberEpi — Digital Outbreak Laboratory",
  description: "Synthetic cyber-epidemiology experiments for homelab research.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>
        <Shell>{children}</Shell>
      </body>
    </html>
  );
}
