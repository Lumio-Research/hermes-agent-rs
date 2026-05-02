// Protocol compatibility test — mirrors the Rust/TypeScript fixture tests
// Loads ws_envelopes.json and validates structure
import * as fs from "fs";
import * as path from "path";

interface WsEnvelope {
  version: number;
  trace_id: string;
  event: { type: string; [key: string]: unknown };
}

describe("protocol fixtures", () => {
  it("ws_envelopes are decodable in React Native SDK", () => {
    const fixturePath = path.resolve(
      __dirname,
      "../../../sdk/protocol-fixtures/ws_envelopes.json"
    );
    const raw = fs.readFileSync(fixturePath, "utf-8");
    const envelopes = JSON.parse(raw) as WsEnvelope[];
    expect(Array.isArray(envelopes)).toBe(true);
    expect(envelopes.length).toBeGreaterThan(0);
    for (const env of envelopes) {
      expect(typeof env.version).toBe("number");
      expect(typeof env.trace_id).toBe("string");
      expect(typeof env.event).toBe("object");
      expect(typeof env.event.type).toBe("string");
    }
  });
});
