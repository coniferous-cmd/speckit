#!/usr/bin/env node

"use strict";

const { execFileSync } = require("child_process");
const path = require("path");
const fs = require("fs");

// Platform and architecture mapping
const PLATFORM_PACKAGES = {
  "darwin-arm64": "@speckit/darwin-arm64",
  "darwin-x64": "@speckit/darwin-x64",
  "linux-x64": "@speckit/linux-x64",
  "win32-x64": "@speckit/win32-x64",
};

function getPlatformPackage() {
  const platform = process.platform;
  const arch = process.arch;

  const key = `${platform}-${arch}`;
  const pkg = PLATFORM_PACKAGES[key];

  if (!pkg) {
    console.error(`error: unsupported platform: ${platform} ${arch}`);
    console.error("Supported platforms:");
    Object.keys(PLATFORM_PACKAGES).forEach((k) => console.error(`  ${k}`));
    process.exit(1);
  }

  return pkg;
}

function getBinaryPath() {
  const pkg = getPlatformPackage();
  const binaryName = process.platform === "win32" ? "speckit.exe" : "speckit";

  try {
    const pkgDir = path.dirname(require.resolve(`${pkg}/package.json`));
    return path.join(pkgDir, binaryName);
  } catch (e) {
    console.error(`error: could not find platform package ${pkg}`);
    console.error("Try reinstalling with: npm install -g speckit");
    process.exit(1);
  }
}

const binaryPath = getBinaryPath();

try {
  const args = process.argv.slice(2);
  execFileSync(binaryPath, args, { stdio: "inherit" });
} catch (e) {
  if (e.status) {
    process.exit(e.status);
  }
  throw e;
}
