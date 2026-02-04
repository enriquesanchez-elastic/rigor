#!/usr/bin/env node
'use strict';

const path = require('path');
const { spawnSync } = require('child_process');

const isWindows = process.platform === 'win32';
const binaryName = isWindows ? 'rigor.exe' : 'rigor';
const binDir = path.resolve(__dirname);
const binaryPath = path.join(binDir, binaryName);

const args = process.argv.slice(2);

function run() {
  if (!require('fs').existsSync(binaryPath)) {
    console.error('rigor: binary not found. Run "npm install" or build from source with "cargo build --release".');
    process.exit(2);
  }
  const result = spawnSync(binaryPath, args, {
    stdio: 'inherit',
    windowsHide: true,
  });
  process.exit(result.status !== null ? result.status : 2);
}

run();
