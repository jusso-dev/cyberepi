"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

const links = [
  ["Laboratory", "/lab"],
  ["Experiments", "/experiments"],
  ["Scenarios", "/scenarios/new"],
  ["Population", "/organisations"],
  ["Pathogens", "/pathogens"],
  ["Controls", "/controls"],
  ["Cluster", "/cluster"],
  ["Notes", "/docs"],
  ["Settings", "/settings"],
];

export function Shell({ children }: { children: React.ReactNode }) {
  const path = usePathname();
  return (
    <div className="min-h-screen md:grid md:grid-cols-[220px_1fr]">
      <aside className="border-b border-line px-6 py-8 md:border-b-0 md:border-r">
        <Link href="/" className="block">
          <p className="font-mono text-[11px] uppercase tracking-[0.22em] text-muted">CyberEpi</p>
          <p className="mt-2 font-serif text-2xl leading-none">Digital Outbreak Laboratory</p>
        </Link>
        <nav className="mt-10 flex flex-wrap gap-x-4 gap-y-2 md:flex-col">
          {links.map(([label, href]) => (
            <Link
              key={href}
              href={href}
              className={`text-sm ${path.startsWith(href) ? "text-ink" : "text-muted"}`}
            >
              {label}
            </Link>
          ))}
        </nav>
      </aside>
      <main className="px-6 py-8 md:px-10">{children}</main>
    </div>
  );
}
