#!/usr/bin/env bun

import { $ } from "bun";
import { chmod, copyFile, mkdir, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";

const distDir = "dist";

const targets = {
  "darwin-x64": { bunTarget: "bun-darwin-x64", binary: "fighorse-darwin-x64" },
  "darwin-arm64": { bunTarget: "bun-darwin-arm64", binary: "fighorse-darwin-arm64" },
  "linux-x64": { bunTarget: "bun-linux-x64", binary: "fighorse-linux-x64" },
  "linux-arm64": { bunTarget: "bun-linux-arm64", binary: "fighorse-linux-arm64" },
};

const targetGroups = {
  macos: ["darwin-x64", "darwin-arm64", "darwin-bundle"],
  linux: ["linux-x64", "linux-arm64"],
  all: ["darwin-x64", "darwin-arm64", "linux-x64", "linux-arm64", "bundle"],
};

const mode = process.argv[2] ?? "bundle";
const compiled = new Map();

async function buildApp() {
  await $`shadow-cljs release app`;
}

async function commandPath(command) {
  const result = await $`command -v ${command}`.quiet().nothrow();
  if (result.exitCode !== 0) {
    return null;
  }
  return result.stdout.toString().trim();
}

async function compileTarget(name) {
  if (compiled.has(name)) {
    return compiled.get(name);
  }

  const target = targets[name];
  if (!target) {
    throw new Error(`Unknown compile target: ${name}`);
  }

  await $`bun build --compile --target=${target.bunTarget} dist/fighorse.js --outfile ${join(distDir, target.binary)}`;
  await chmod(join(distDir, target.binary), 0o755);
  compiled.set(name, join(distDir, target.binary));
  return compiled.get(name);
}

async function packageBinary(name, sourceBinary) {
  const packageDir = join(distDir, `package-${name}`);
  const packageBinaryPath = join(packageDir, "fighorse");
  const archive = join(distDir, `fighorse-${name}.tar.gz`);

  await rm(packageDir, { recursive: true, force: true });
  await rm(archive, { force: true });
  await mkdir(packageDir, { recursive: true });
  await copyFile(sourceBinary, packageBinaryPath);
  await chmod(packageBinaryPath, 0o755);
  await $`tar -C ${packageDir} -czf ${archive} fighorse`;
  await rm(packageDir, { recursive: true, force: true });

  console.log(`Wrote ${archive}`);
}

async function packageTarget(name) {
  const sourceBinary = await compileTarget(name);
  await packageBinary(name, sourceBinary);
}

function launcherScript() {
  return `#!/bin/sh
set -eu

os="$(uname -s)"
arch="$(uname -m)"

case "$os:$arch" in
  Darwin:x86_64) target="darwin-x64" ;;
  Darwin:arm64) target="darwin-arm64" ;;
  Linux:x86_64|Linux:amd64) target="linux-x64" ;;
  Linux:aarch64|Linux:arm64) target="linux-arm64" ;;
  *)
    echo "Unsupported platform for fighorse: $os $arch" >&2
    exit 1
    ;;
esac

dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
exec "$dir/bin/fighorse-$target" "$@"
`;
}

async function packageBundle(name, targetNames) {
  const packageDir = join(distDir, `package-${name}`);
  const archive = join(distDir, `fighorse-${name}.tar.gz`);
  const binDir = join(packageDir, "bin");

  await rm(packageDir, { recursive: true, force: true });
  await rm(archive, { force: true });
  await mkdir(binDir, { recursive: true });

  const launcher = join(packageDir, "fighorse");
  await writeFile(launcher, launcherScript());
  await chmod(launcher, 0o755);

  for (const targetName of targetNames) {
    const sourceBinary = await compileTarget(targetName);
    const bundledBinary = join(binDir, `fighorse-${targetName}`);
    await copyFile(sourceBinary, bundledBinary);
    await chmod(bundledBinary, 0o755);
  }

  await $`tar -C ${packageDir} -czf ${archive} fighorse bin`;
  await rm(packageDir, { recursive: true, force: true });

  console.log(`Wrote ${archive}`);
}

async function packageUniversalDarwin() {
  if (process.platform !== "darwin") {
    throw new Error("darwin-universal packaging requires macOS because it uses lipo.");
  }

  const lipo = (await commandPath("lipo")) ?? (await commandPath("llvm-lipo"));
  if (!lipo) {
    throw new Error("darwin-universal packaging requires lipo or llvm-lipo.");
  }

  const x64Binary = await compileTarget("darwin-x64");
  const arm64Binary = await compileTarget("darwin-arm64");
  const universalBinary = join(distDir, "fighorse-darwin-universal");

  await rm(universalBinary, { force: true });
  await $`${lipo} -create -output ${universalBinary} ${x64Binary} ${arm64Binary}`;
  await chmod(universalBinary, 0o755);
  await packageBinary("darwin-universal", universalBinary);
}

async function main() {
  const packageTargets = targetGroups[mode] ?? [mode];

  await buildApp();

  for (const target of packageTargets) {
    if (target === "darwin-universal") {
      await packageUniversalDarwin();
    } else if (target === "darwin-bundle") {
      await packageBundle("darwin-bundle", ["darwin-x64", "darwin-arm64"]);
    } else if (target === "bundle") {
      await packageBundle("multi-platform", ["darwin-x64", "darwin-arm64", "linux-x64", "linux-arm64"]);
    } else {
      await packageTarget(target);
    }
  }
}

main().catch((err) => {
  console.error(err?.message ?? err);
  process.exit(1);
});
