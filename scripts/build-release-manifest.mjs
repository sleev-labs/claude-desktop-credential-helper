#!/usr/bin/env node
/**
 * Builds the release manifest consumed by the sleev CLI's credential-helper
 * acquisition. Mirrors sleev's cli/scripts/build-release-manifest.mjs: for
 * each platform archive in --release-dir, records {url, sha256, size}, and
 * writes a JSON document with `artifacts`, `channel`, `commit`,
 * `publishedAt`, and `version`. Keys are emitted sorted so the output is
 * byte-stable for a given input.
 */

import { createHash } from "node:crypto";
import { readFileSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const PLATFORMS = {
    "claude-desktop-cred-linux-x64.tar.gz": "linux-x64",
    "claude-desktop-cred-linux-arm64.tar.gz": "linux-arm64",
    "claude-desktop-cred-darwin-x64.tar.gz": "darwin-x64",
    "claude-desktop-cred-darwin-arm64.tar.gz": "darwin-arm64",
    "claude-desktop-cred-win32-x64.zip": "win32-x64",
    "claude-desktop-cred-win32-arm64.zip": "win32-arm64",
};

function parseArgs(argv) {
    const result = {};
    for (let i = 0; i < argv.length; i += 1) {
        const arg = argv[i];
        if (!arg.startsWith("--")) continue;
        const key = arg.slice(2);
        const value = argv[i + 1] ?? "";
        result[key] = value;
        i += 1;
    }
    return result;
}

function sha256(path) {
    return createHash("sha256").update(readFileSync(path)).digest("hex");
}

const args = parseArgs(process.argv.slice(2));
const required = ["version", "channel", "commit", "base-url", "release-dir", "output"];
for (const key of required) {
    if (!args[key]) {
        process.stderr.write(`Missing required argument: --${key}\n`);
        process.exit(1);
    }
}

const baseUrl = args["base-url"].replace(/\/+$/, "");
const artifacts = {};
for (const [archiveName, platform] of Object.entries(PLATFORMS)) {
    const archivePath = join(args["release-dir"], archiveName);
    artifacts[platform] = {
        url: `${baseUrl}/${archiveName}`,
        sha256: sha256(archivePath),
        size: statSync(archivePath).size,
    };
}

const manifest = {
    artifacts,
    channel: args.channel,
    commit: args.commit,
    publishedAt: new Date().toISOString(),
    version: args.version,
};

const sortedKeys = Object.keys(manifest).sort();
const sorted = Object.fromEntries(sortedKeys.map((key) => [key, manifest[key]]));
const artifactKeys = Object.keys(sorted.artifacts).sort();
sorted.artifacts = Object.fromEntries(
    artifactKeys.map((key) => [key, sorted.artifacts[key]]),
);

writeFileSync(args.output, JSON.stringify(sorted, null, 2) + "\n");
