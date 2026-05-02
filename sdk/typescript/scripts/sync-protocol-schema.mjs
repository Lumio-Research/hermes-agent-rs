import fs from "node:fs/promises";
import path from "node:path";

const apiBase = process.env.HERMES_SCHEMA_BASE || "http://127.0.0.1:8787";
const outFile = path.resolve("src/generated/ws-envelope.schema.json");

const url = `${apiBase}/v1/protocol/schema`;
const resp = await fetch(url);
if (!resp.ok) {
  throw new Error(`failed to fetch schema: ${resp.status} ${await resp.text()}`);
}
const payload = await resp.json();
await fs.mkdir(path.dirname(outFile), { recursive: true });
await fs.writeFile(outFile, JSON.stringify(payload, null, 2));
console.log(`wrote schema -> ${outFile}`);
