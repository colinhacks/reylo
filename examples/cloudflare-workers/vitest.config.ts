import { cloudflareTest, readD1Migrations } from "@cloudflare/vitest-plugin";
import { defineConfig } from "vitest/config";
import reylo from "reylo/vite";

// The tests run inside workerd, with the bindings of wrangler.jsonc and a D1 database of their own,
// which test/apply-migrations.ts fills from reylo/migrations.
export default defineConfig(async () => ({
  plugins: [
    reylo(),
    cloudflareTest({
      wrangler: { configPath: "./wrangler.jsonc" },
      miniflare: { bindings: { MIGRATIONS: await readD1Migrations("reylo/migrations") } },
    }),
  ],
  test: { setupFiles: ["./test/apply-migrations.ts"] },
}));
