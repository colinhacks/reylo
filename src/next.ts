// The Next.js integration, `export default withReylo(nextConfig)` in next.config.ts: the loader on
// the server for Turbopack and webpack, and a type check that runs the content mapper inside
// `next build`.
//
// Since 16.3, `next build` type-checks with the project's own `tsc` (`experimental.useTypeScriptCli`,
// on by default), which TypeScript 7 is, but with arguments of its own and no `--runExternalCode`
// among them, and TypeScript takes that flag from the command line alone. So its check never runs
// the mapper and fails on every `.sql` import (TS2307, and TS18068 on tsconfig.json). The wrapper
// turns that check off and runs the same one with the flag in `compiler.runAfterProductionCompile`,
// which runs under Turbopack and webpack alike, after the compile and before Next's own type check
// and the prerender (wiki/nextjs.md).
import { spawn } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";

type AfterCompile = (metadata: { projectDir: string; distDir: string }) => Promise<void>;

// The part of Next's config the wrapper reads or sets, so that reylo has no dependency on next.
// Each field's type admits Next's own, and the rest of the config passes through as it is. Every
// level is intersected with `object`: one whose properties are all optional is a weak type, which
// a literal sharing none of them, as `{ reactStrictMode: true }` or `{ turbopack: { root } }`,
// fails, reported as excess properties. An index signature would not do: Next's own `NextConfig`
// and its parts are interfaces, which have none, so they are assignable to one only when its value
// is `any`, which lets any value through.
type NextConfig = object & {
  typescript?: object & { ignoreBuildErrors?: boolean; tsconfigPath?: string };
  turbopack?: object & { rules?: Record<string, unknown> };
  webpack?: ((config: any, context: any) => any) | null;
  compiler?: object & { runAfterProductionCompile?: AfterCompile };
};

export function withReylo<Config extends NextConfig>(config: Config = {} as Config): Config {
  const { typescript, turbopack, webpack, compiler } = config;
  // A project that turned Next's type check off has turned Reylo's off with it.
  const check = !typescript?.ignoreBuildErrors;
  return {
    ...config,
    turbopack: {
      ...turbopack,
      // Each .sql file becomes a module that runs its statement, on the server only: a client
      // component that imports one fails the build. A rule of the project's own for *.sql wins.
      rules: { "*.sql": { condition: { not: "browser" }, loaders: ["reylo/turbopack"], as: "*.js" }, ...turbopack?.rules },
    },
    webpack(webpackConfig: any, context: any) {
      const out = webpack ? webpack(webpackConfig, context) : webpackConfig;
      if (context.isServer) out.module.rules.push({ test: /\.sql$/, use: "reylo/webpack" });
      return out;
    },
    ...(check && {
      typescript: { ...typescript, ignoreBuildErrors: true },
      compiler: {
        ...compiler,
        async runAfterProductionCompile(metadata: { projectDir: string; distDir: string }) {
          await compiler?.runAfterProductionCompile?.(metadata);
          await typeCheck(metadata.projectDir, resolve(metadata.projectDir, typescript?.tsconfigPath ?? "tsconfig.json"));
        },
      },
    }),
  } as Config;
}

// Next's own check (`next/dist/lib/typescript/runTypeCheckCli`), with `--runExternalCode`: the
// project's TypeScript, run with this process's Node from the project directory.
async function typeCheck(dir: string, tsconfig: string): Promise<void> {
  const require = createRequire(join(dir, "package.json"));
  let manifest: string;
  try {
    manifest = require.resolve("typescript/package.json");
  } catch {
    throw stackless("Reylo's type check needs TypeScript 7.1 or later, whose content mappers type .sql imports, and the project has no TypeScript.");
  }
  const { version } = JSON.parse(readFileSync(manifest, "utf8")) as { version: string };
  const [major = 0, minor = 0] = version.split(".").map(Number);
  if (major < 7 || (major === 7 && minor < 1)) {
    throw stackless(`Reylo's type check needs TypeScript 7.1 or later, whose content mappers type .sql imports; the project has TypeScript ${version}.`);
  }
  // Next writes next-env.d.ts after this hook; `next typegen` writes it on a first build, for a
  // project that declares an image import through it.
  if (!existsSync(join(dir, "next-env.d.ts"))) {
    const next = require.resolve("next/package.json");
    const bin = (JSON.parse(readFileSync(next, "utf8")) as { bin: { next: string } }).bin.next;
    const typegen = await run([join(dirname(next), bin), "typegen", dir], dir);
    if (typegen.code !== 0) throw stackless(`next typegen failed:\n${typegen.output}`);
  }
  const tsc = join(dirname(manifest), "lib", "tsc.js");
  const args = ["--project", tsconfig, "--noEmit", "--declarationMap", "false", "--emitDeclarationOnly", "false", "--runExternalCode"];
  const { code, output } = await run([tsc, ...args], dir);
  // Next's spinner pauses for console output, and not for a write to stdout.
  if (output.trim()) console.log(output.trimEnd());
  if (code !== 0) throw stackless("Failed to type check (tsc --runExternalCode).");
}

// A child Node process's exit code and its output, stdout and stderr in the order they came. A
// piped tsc prints no colors unless FORCE_COLOR asks, which Next sets for its own check too.
function run(args: string[], cwd: string): Promise<{ code: number; output: string }> {
  return new Promise((done, fail) => {
    const child = spawn(process.execPath, args, {
      cwd,
      stdio: ["ignore", "pipe", "pipe"],
      env: { ...process.env, ...(process.stdout.isTTY && { FORCE_COLOR: "1" }) },
    });
    let output = "";
    child.stdout.setEncoding("utf8").on("data", (chunk: string) => (output += chunk));
    child.stderr.setEncoding("utf8").on("data", (chunk: string) => (output += chunk));
    child.on("error", fail);
    child.on("close", (code) => done({ code: code ?? 1, output }));
  });
}

// `next build` prints a failed hook's error whole: a stack would point into Reylo, not at the
// errors, which tsc has printed above it.
function stackless(message: string): Error {
  const error = new Error(message);
  error.stack = message;
  return error;
}
