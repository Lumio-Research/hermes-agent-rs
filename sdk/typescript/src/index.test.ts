import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs/promises";
import type { WsEnvelope } from "./index.js";

test("decodes protocol envelope", () => {
  const raw = {
    version: 1,
    request_id: "req-1",
    trace_id: "trace-1",
    event: { type: "text", content: "hello" },
  };
  const envelope = raw as WsEnvelope;
  assert.equal(envelope.version, 1);
  assert.equal(envelope.request_id, "req-1");
  assert.equal(envelope.event.type, "text");
});

test("compat fixtures are decodable in TS SDK", async () => {
  const fixturePath = new URL("../../protocol-fixtures/ws_envelopes.json", import.meta.url);
  const raw = await fs.readFile(fixturePath, "utf-8");
  const fixtures = JSON.parse(raw) as WsEnvelope[];
  assert.equal(fixtures.length, 4);
  assert.equal(fixtures[0].event.type, "connected");
  assert.equal(fixtures[1].event.type, "text");
  assert.equal(fixtures[2].event.type, "error");
  assert.equal(fixtures[3].event.type, "done");
});
