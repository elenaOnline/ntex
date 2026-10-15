#!/usr/bin/env node
// node run-probes.mjs <appname> <out.json> [LATEXSUITE_DIR]
// Types every probe into the given ntex (headless) and writes the results.
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { probes } from "./probes.mjs";
const HERE = path.dirname(fileURLToPath(import.meta.url));
const [app, out, lsdir] = process.argv.slice(2);
const list = probes();
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "ntex-probes-"));
fs.writeFileSync(path.join(tmp, "jobs.json"), JSON.stringify({ examples: list.map((p) => ({ id: p.id, before: p.before, keys: p.keys, select: p.select, ft: p.ft })) }));
const env = { ...process.env, TEXNVIM_APPNAME: app, TEXNVIM_NO_COMPILE: "1", TEXNVIM_NO_REFERENCE: "1" };
if (lsdir) env.LATEXSUITE_DIR = lsdir;
delete env.TEXNVIM_SNIPPETS;
const t0 = Date.now();
const res = spawnSync("nvim", ["--clean", "--headless", "-l", path.join(HERE, "../../reference/verify.lua"), path.join(tmp, "jobs.json"), path.join(tmp, "out.json")], { env, encoding: "utf8", timeout: 1800000 });
if (res.status !== 0) { console.error(res.stderr); process.exit(1); }
const data = JSON.parse(fs.readFileSync(path.join(tmp, "out.json"), "utf8"));
fs.rmSync(tmp, { recursive: true, force: true });
fs.writeFileSync(out, JSON.stringify({ app, taken: new Date().toISOString(), results: data.examples }));
console.log(`${app}: ${list.length} probes in ${((Date.now() - t0) / 1000).toFixed(0)} s → ${out}`);
