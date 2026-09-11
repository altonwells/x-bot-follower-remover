import { build } from "esbuild";
import { mkdir, copyFile, readdir, cp } from "node:fs/promises";
if (process.argv.includes("--test")) {
  await mkdir(".test-build", { recursive: true });
  const files = (await readdir("tests")).filter((f) => f.endsWith(".test.ts"));
  await build({
    entryPoints: files.map((f) => `tests/${f}`),
    outdir: ".test-build",
    outExtension: { ".js": ".mjs" },
    bundle: true,
    platform: "node",
    external: ["esbuild"],
    format: "esm",
    target: "node22",
  });
} else {
  await mkdir("dist", { recursive: true });
  await build({
    entryPoints: ["src/background.ts", "src/options.ts"],
    outdir: "dist",
    bundle: true,
    format: "esm",
    target: "chrome116",
    sourcemap: true,
  });
  await cp("icons", "dist/icons", { recursive: true });
  for (const file of [
    "manifest.json",
    "options.html",
    "options.css",
    "THIRD_PARTY_LICENSES.txt",
  ])
    await copyFile(file, `dist/${file}`);
}
