import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SynapsisClient } from "../src/index";

describe("SynapsisClient.callTool", () => {
  const fetchMock = vi.fn();

  beforeEach(() => {
    fetchMock.mockReset();
    vi.stubGlobal("fetch", fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("sends a JSON-RPC tools/call request and returns the first text result", async () => {
    fetchMock.mockResolvedValue({
      json: async () => ({ result: { content: [{ text: "saved" }] } }),
    });
    const client = new SynapsisClient({ baseUrl: "https://synapsis.test" });

    await expect(client.callTool("mem_save", { title: "note" })).resolves.toBe(
      "saved"
    );

    expect(fetchMock).toHaveBeenCalledWith(
      "https://synapsis.test/message",
      expect.objectContaining({
        method: "POST",
        headers: { "Content-Type": "application/json" },
      })
    );
    const request = JSON.parse(fetchMock.mock.calls[0][1].body);
    expect(request).toEqual({
      jsonrpc: "2.0",
      method: "tools/call",
      params: { name: "mem_save", arguments: { title: "note" } },
      id: 1,
    });
  });

  it("sends the configured API key as a bearer token", async () => {
    fetchMock.mockResolvedValue({ json: async () => ({ result: {} }) });
    const client = new SynapsisClient({
      baseUrl: "https://synapsis.test",
      apiKey: "test-key",
    });

    await client.callTool("db_integrity");

    expect(fetchMock.mock.calls[0][1].headers).toEqual({
      "Content-Type": "application/json",
      Authorization: "Bearer test-key",
    });
  });

  it("rejects JSON-RPC errors with the server message", async () => {
    fetchMock.mockResolvedValue({
      json: async () => ({ error: { message: "permission denied" } }),
    });
    const client = new SynapsisClient();

    await expect(client.callTool("mem_save")).rejects.toThrow(
      "permission denied"
    );
  });
});
