#!/usr/bin/env node
// Syncs the version from package.json to Cargo.toml and tauri.conf.json.
// Run: node scripts/sync-version.mjs

import { readFileSync, writeFileSync } from "fs";
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const pkg = JSON.parse(readFileSync(resolve(root, "package.json"), "utf-8"));
const version = pkg.version;

// --- Cargo.toml (line-level replace) ---
const cargoPath = resolve(root, "src-tauri/Cargo.toml");
const cargo = readFileSync(cargoPath, "utf-8");
const updatedCargo = cargo.replace(
  /^version\s*=\s*".*"/m,
  `version = "${version}"`,
);
writeFileSync(cargoPath, updatedCargo);

// --- tauri.conf.json ---
const tauriPath = resolve(root, "src-tauri/tauri.conf.json");
const tauri = JSON.parse(readFileSync(tauriPath, "utf-8"));
tauri.version = version;
writeFileSync(tauriPath, JSON.stringify(tauri, null, 2) + "\n");

console.log(`Synced version ${version} → Cargo.toml, tauri.conf.json`);
