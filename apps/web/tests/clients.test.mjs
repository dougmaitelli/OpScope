import assert from "node:assert/strict";
import { test } from "node:test";
import { HttpClient } from "../src/clients.ts";

test("provider 401 errors do not expire the web session", async () => {
  const events = [];
  const previous = globalThis.dispatchEvent;
  globalThis.dispatchEvent = (event) => {
    events.push(event.type);
    return true;
  };
  try {
    const client = new HttpClient(
      async () =>
        new Response(JSON.stringify({ message: "Invalid provider credential" }), { status: 401 }),
    );
    await assert.rejects(
      client.connectSource({ sourceId: "github", configuration: {}, credential: "invalid" }),
      /Invalid provider credential/,
    );
    await assert.rejects(client.listWorkflows(), /Invalid provider credential/);
    assert.deepEqual(events, []);
  } finally {
    globalThis.dispatchEvent = previous;
  }
});

test("only an explicit middleware 401 expires the web session", async () => {
  const events = [];
  const previous = globalThis.dispatchEvent;
  globalThis.dispatchEvent = (event) => {
    events.push(event.type);
    return true;
  };
  try {
    const client = new HttpClient(
      async () =>
        new Response(null, {
          status: 401,
          headers: { "x-opsscope-session-error": "unauthenticated" },
        }),
    );
    await assert.rejects(client.listWorkflows(), /401/);
    assert.deepEqual(events, ["opsscope:unauthorized"]);
    events.length = 0;
    const forbidden = new HttpClient(
      async () =>
        new Response(null, {
          status: 403,
          headers: { "x-opsscope-session-error": "unauthenticated" },
        }),
    );
    await assert.rejects(forbidden.listWorkflows(), /403/);
    assert.deepEqual(events, []);
  } finally {
    globalThis.dispatchEvent = previous;
  }
});
