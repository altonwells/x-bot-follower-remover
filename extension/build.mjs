import { build } from "esbuild";
import {
  mkdir,
  copyFile,
  readdir,
  cp,
  readFile,
  writeFile,
} from "node:fs/promises";
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
    entryPoints: ["src/background.ts", "src/options.ts", "src/manager.ts"],
    outdir: "dist",
    bundle: true,
    format: "esm",
    target: "chrome116",
    sourcemap: true,
  });
  await cp("icons", "dist/icons", { recursive: true });
  for (const file of [
    "manifest.json",
    "options.css",
    "THIRD_PARTY_LICENSES.txt",
  ])
    await copyFile(file, `dist/${file}`);
}

if (!process.argv.includes("--test")) {
  const { version } = JSON.parse(await readFile("manifest.json", "utf8"));
  await writeFile(
    "dist/options.html",
    (await readFile("options.html", "utf8")).replace(
      "__EXTENSION_VERSION__",
      version,
    ),
  );
}
