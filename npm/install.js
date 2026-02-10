#!/usr/bin/env node
'use strict';

const fs = require('fs');
const path = require('path');
const os = require('os');
const https = require('https');
const { execSync } = require('child_process');

const PACKAGE_ROOT = path.resolve(__dirname);
const BIN_DIR = path.join(PACKAGE_ROOT, 'bin');
const BINARY_NAME = process.platform === 'win32' ? 'rigor.exe' : 'rigor';
const BINARY_PATH = path.join(BIN_DIR, BINARY_NAME);

// GitHub releases base URL (update when publishing releases)
const REPO = 'rigor-dev/rigor';
const VERSION = require(path.join(PACKAGE_ROOT, 'package.json')).version;

// Map Node.js platform/arch to CI artifact naming conventions (Rust targets)
const PLATFORM_MAP = {
  'darwin-arm64': 'macos-aarch64',
  'darwin-x64': 'macos-x86_64',
  'linux-arm64': 'linux-aarch64',
  'linux-x64': 'linux-x86_64',
  'win32-arm64': 'windows-aarch64',
  'win32-x64': 'windows-x86_64',
};

function getPlatformKey() {
  const key = `${process.platform}-${process.arch}`;
  return PLATFORM_MAP[key] || null;
}

function downloadBinary(url) {
  return new Promise((resolve, reject) => {
    const file = fs.createWriteStream(BINARY_PATH, { mode: 0o755 });
    https.get(url, { redirect: 'follow' }, (res) => {
      if (res.statusCode === 302 || res.statusCode === 301) {
        const redirect = res.headers.location;
        return https.get(redirect, { redirect: 'follow' }, (r) => r.pipe(file).on('finish', resolve).on('error', reject));
      }
      res.pipe(file).on('finish', resolve).on('error', reject);
    }).on('error', reject);
  });
}

function tryDownloadPrebuilt() {
  const key = getPlatformKey();
  if (!key) return Promise.resolve(false);
  const ext = process.platform === 'win32' ? '.exe' : '';
  const url = `https://github.com/${REPO}/releases/download/v${VERSION}/rigor-${key}${ext}`;
  return downloadBinary(url).then(() => true).catch(() => false);
}

function useLocalBuild() {
  const parent = path.resolve(PACKAGE_ROOT, '..');
  const cargoToml = path.join(parent, 'Cargo.toml');
  const releaseBinary = path.join(parent, 'target', 'release', BINARY_NAME);
  if (fs.existsSync(cargoToml) && fs.existsSync(releaseBinary)) {
    fs.copyFileSync(releaseBinary, BINARY_PATH);
    fs.chmodSync(BINARY_PATH, 0o755);
    return true;
  }
  return false;
}

function ensureBinDir() {
  if (!fs.existsSync(BIN_DIR)) fs.mkdirSync(BIN_DIR, { recursive: true });
}

async function main() {
  ensureBinDir();
  if (fs.existsSync(BINARY_PATH)) return;

  // Prefer local build when developing (npm package inside rigor repo)
  if (useLocalBuild()) {
    console.log('rigor-cli: using local build');
    return;
  }

  // Try to download prebuilt binary from GitHub releases
  const ok = await tryDownloadPrebuilt();
  if (ok) {
    console.log('rigor-cli: installed prebuilt binary');
    return;
  }

  console.error('rigor-cli: failed to install prebuilt binary for this platform.');
  console.error('Install manually with: cargo install --path . (from rigor repo)');
  process.exit(1);
}

main().catch((err) => {
  console.error('rigor-cli: installation failed:', err.message || err);
  process.exit(1);
});
