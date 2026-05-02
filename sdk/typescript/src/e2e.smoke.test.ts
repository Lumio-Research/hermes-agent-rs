import test from "node:test";
import assert from "node:assert/strict";
import { HermesAppSdk } from "./index.js";

test("protocol e2e smoke (optional)", async (t) => {
  const base = process.env.HERMES_E2E_BASE;
  if (!base) {
    t.skip("set HERMES_E2E_BASE to run protocol e2e smoke test");
    return;
  }

  const sdk = new HermesAppSdk(base, process.env.HERMES_E2E_TOKEN);
  const session = await sdk.createSession("e2e-smoke");
  assert.ok(session.id);
  const events = sdk.sendMessageStream(session.id, "ping from sdk test");
  let receivedConnected = false;

  await Promise.race([
    (async () => {
      for await (const event of events) {
        if (event.event.type === "connected") {
          receivedConnected = true;
          break;
        }
      }
    })(),
    new Promise((_, reject) =>
      setTimeout(
        () => reject(new Error("timed out waiting for connected event")),
        10_000,
      ),
    ),
  ]);

  assert.equal(receivedConnected, true);
});
