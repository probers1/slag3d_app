#!/usr/bin/env node
// Sync version from latest git tag to package.json and tauri.conf.json
// Usage: node scripts/sync-version.js

import { readFileSync, writeFileSync } from 'fs';
import { execSync } from 'child_process';
import { resolve } from 'path';

const ROOT = resolve(import.meta.dirname, '..');
const PKG_PATH = resolve(ROOT, 'package.json');
const TAURI_CONF_PATH = resolve(ROOT, 'src-tauri/tauri.conf.json');

function getLatestTag() {
  try {
    // Get latest semver tag (v0.0.10 > v0.0.9)
    const tags = execSync('git tag -l "v*"', { cwd: ROOT, encoding: 'utf8' })
      .trim()
      .split('\n')
      .filter(Boolean);

    if (tags.length === 0) {
      console.warn('No version tags found, using default 0.0.0');
      return '0.0.0';
    }

    // Sort by semver (simple version for vX.Y.Z format)
    tags.sort((a, b) => {
      const pa = a.slice(1).split('.').map(Number);
      const pb = b.slice(1).split('.').map(Number);
      for (let i = 0; i < 3; i++) {
        if (pa[i] !== pb[i]) return pa[i] - pb[i];
      }
      return 0;
    });

    const latest = tags[tags.length - 1];
    console.log(`Latest git tag: ${latest}`);
    return latest.slice(1); // Remove 'v' prefix
  } catch (e) {
    console.error('Failed to get git tags:', e.message);
    return '0.0.0';
  }
}

function updatePackageJson(version) {
  const pkg = JSON.parse(readFileSync(PKG_PATH, 'utf8'));
  pkg.version = version;
  writeFileSync(PKG_PATH, JSON.stringify(pkg, null, 2) + '\n');
  console.log(`Updated package.json version to ${version}`);
}

function updateTauriConf(version) {
  const conf = JSON.parse(readFileSync(TAURI_CONF_PATH, 'utf8'));
  conf.version = version;
  writeFileSync(TAURI_CONF_PATH, JSON.stringify(conf, null, 2) + '\n');
  console.log(`Updated tauri.conf.json version to ${version}`);
}

const version = getLatestTag();
updatePackageJson(version);
updateTauriConf(version);