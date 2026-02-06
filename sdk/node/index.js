/**
 * @rigor/sdk — Node.js wrapper for Rigor's programmatic API.
 * Requires the `rigor` binary on PATH (or set RIGOR_BIN).
 *
 * Usage:
 *   const { analyze } = require('@rigor/sdk');
 *   const result = await analyze('path/to/test.test.ts');
 *   const result = await analyze({ stdin: sourceCode, filename: 'stdin.test.ts' });
 */

const { spawn } = require('child_process');
const path = require('path');

const RIGOR_BIN = process.env.RIGOR_BIN || 'rigor';

/**
 * Run rigor and return parsed JSON result(s).
 * @param {string|{ stdin: string, filename?: string }} input - File path, or { stdin, filename } for in-memory source
 * @param {{ config?: string, threshold?: number }} options - Optional config path and threshold
 * @returns {Promise<object|object[]>} Analysis result (single object for one file/stdin, or array/summary for dir)
 */
function analyze(input, options = {}) {
  return new Promise((resolve, reject) => {
    const args = ['--json'];
    if (options.config) args.push('--config', options.config);
    if (options.threshold != null) args.push('--threshold', String(options.threshold));

    let stdin = null;
    if (typeof input === 'object' && input !== null && typeof input.stdin === 'string') {
      args.push('--stdin');
      if (input.filename) args.push('--stdin-filename', input.filename);
      stdin = input.stdin;
    } else if (typeof input === 'string') {
      args.push(input);
    } else {
      reject(new Error('input must be a file path string or { stdin: string, filename?: string }'));
      return;
    }

    const proc = spawn(RIGOR_BIN, args, {
      stdio: [stdin ? 'pipe' : 'ignore', 'pipe', 'pipe'],
      shell: process.platform === 'win32',
    });

    let stdout = '';
    let stderr = '';
    proc.stdout.setEncoding('utf8').on('data', (chunk) => { stdout += chunk; });
    proc.stderr.setEncoding('utf8').on('data', (chunk) => { stderr += chunk; });

    if (stdin) {
      proc.stdin.write(stdin, () => proc.stdin.end());
    }

    proc.on('error', (err) => reject(err));
    proc.on('close', (code) => {
      try {
        const out = stdout.trim();
        const parsed = out ? JSON.parse(out) : null;
        if (code !== 0 && code !== 1) {
          reject(new Error(stderr || `rigor exited with code ${code}`));
          return;
        }
        resolve(parsed);
      } catch (e) {
        reject(new Error(stderr || stdout || e.message));
      }
    });
  });
}

/**
 * Analyze test source from a string (convenience for stdin).
 * @param {string} source - Test file content
 * @param {{ filename?: string }} options - Optional virtual filename (default: stdin.test.ts)
 */
async function analyzeSource(source, options = {}) {
  return analyze({ stdin: source, filename: options.filename || 'stdin.test.ts' }, options);
}

module.exports = { analyze, analyzeSource };
