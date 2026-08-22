"use strict";

const { execFileSync } = require("child_process");
const path = require("path");

// Platform and architecture mapping
const PLATFORM_PACKAGES = {
  "darwin-arm64": "@speckit/darwin-arm64",
  "darwin-x64": "@speckit/darwin-x64",
  "linux-x64": "@speckit/linux-x64",
  "linux-arm64": "@speckit/linux-arm64",
  "win32-x64": "@speckit/win32-x64",
};

function getPlatformPackage() {
  const platform = process.platform;
  const arch = process.arch;
  const key = `${platform}-${arch}`;
  return PLATFORM_PACKAGES[key];
}

function verifyInstallation() {
  const pkg = getPlatformPackage();

  if (!pkg) {
    console.error(`\x1b[31merror: unsupported platform: ${process.platform} ${process.arch}\x1b[0m`);
    console.error("Supported platforms:");
    Object.keys(PLATFORM_PACKAGES).forEach((k) => console.error(`  ${k}`));
    process.exit(1);
  }

  const binaryName = process.platform === "win32" ? "speckit.exe" : "speckit";

  try {
    const pkgDir = path.dirname(require.resolve(`${pkg}/package.json`));
    const binaryPath = path.join(pkgDir, binaryName);

    // Test that the binary works
    const result = execFileSync(binaryPath, ["--version"], {
      encoding: "utf8",
      stdio: ["pipe", "pipe", "pipe"],
    });

    console.log(`\x1b[32mspeckit installed successfully: ${result.trim()}\x1b[0m`);
  } catch (e) {
    console.error(`\x1b[31merror: failed to verify speckit installation\x1b[0m`);
    console.error(`Platform package: ${pkg}`);
    console.error("Try reinstalling with: npm install -g speckit");
    process.exit(1);
  }
}

verifyInstallation();
