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

function getPlatformKey() {
  const platform = process.platform;
  const arch = process.arch;
  if (platform === 'darwin') return arch === 'arm64' ? 'darwin-arm64' : 'darwin-x64';
  if (platform === 'linux') return arch === 'arm64' ? 'linux-arm64' : 'linux-x64';
  if (platform === 'win32') return arch === 'arm64' ? 'win32-arm64' : 'win32-x64';
  return null;
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
  if (!key) return false;
  const url = `https://github.com/${REPO}/releases/download/v${VERSION}/rigor-${key}${process.platform === 'win32' ? '.exe' : ''}`;
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

function main() {
  ensureBinDir();
  if (fs.existsSync(BINARY_PATH)) return;

  // Prefer local build when developing (npm package inside rigor repo)
  if (useLocalBuild()) {
    console.log('rigor-cli: using local build');
    return;
  }

  // Try to download prebuilt binary from GitHub releases
  tryDownloadPrebuilt().then((ok) => {
    if (ok) {
      console.log('rigor-cli: installed prebuilt binary');
      return;
    }
    console.warn('rigor-cli: no prebuilt binary for this platform. Install with: cargo install --path . (from rigor repo)');
  });
}

main();
