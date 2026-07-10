/**
 * Synapsis Client SDK
 * TypeScript SDK for consuming the Synapsis MCP Server API.
 *
 * @packageDocumentation
 */

export interface SynapsisConfig {
  /** MCP server URL (default: http://127.0.0.1:7438) */
  baseUrl?: string;
  /** API key for authenticated endpoints */
  apiKey?: string;
}

export interface Observation {
  id: number;
  title: string;
  content: string;
  project?: string;
  type: string;
  scope: string;
  created_at: number;
  updated_at: number;
}

export interface SessionInfo {
  session_id: string;
  project: string;
  started_at: number;
  observation_count: number;
  is_active: boolean;
}

export interface McpResponse<T> {
  result?: T;
  error?: { message: string };
}

export class SynapsisClient {
  private baseUrl: string;
  private apiKey?: string;
  private requestId = 1;

  constructor(config: SynapsisConfig = {}) {
    this.baseUrl = config.baseUrl ?? "http://127.0.0.1:7438";
    this.apiKey = config.apiKey;
  }

  /** Call an MCP tool. */
  async callTool<T = unknown>(
    name: string,
    args: Record<string, unknown> = {}
  ): Promise<T> {
    const headers: Record<string, string> = {
      "Content-Type": "application/json",
    };
    if (this.apiKey) headers["Authorization"] = `Bearer ${this.apiKey}`;

    const res = await fetch(`${this.baseUrl}/message`, {
      method: "POST",
      headers,
      body: JSON.stringify({
        jsonrpc: "2.0",
        method: "tools/call",
        params: { name, arguments: args },
        id: this.requestId++,
      }),
    });

    const json: McpResponse<{ content: { text?: string }[] }> =
      await res.json();
    if (json.error) throw new Error(json.error.message);
    return json.result?.content?.[0]?.text as T;
  }

  // ── Memory ─────────────────────────────────────────

  async saveMemory(
    title: string,
    content: string,
    opts?: { project?: string; type?: string; session_id?: string }
  ) {
    return this.callTool("mem_save", { title, content, ...opts });
  }

  async searchMemory(query: string, limit = 10) {
    return this.callTool<Observation[]>("mem_search", { query, limit });
  }

  async getContext(limit = 5) {
    return this.callTool("mem_context", { limit });
  }

  async getStats() {
    return this.callTool("mem_stats");
  }

  // ── Sessions ───────────────────────────────────────

  async startSession(
    project: string,
    opts?: { agent_id?: string; directory?: string }
  ) {
    return this.callTool("mem_session_start", { project, ...opts });
  }

  async endSession(sessionId: string, summary?: string) {
    return this.callTool("mem_session_end", {
      session_id: sessionId,
      ...(summary ? { summary } : {}),
    });
  }

  async listSharedSessions() {
    return this.callTool("shared_sessions_list");
  }

  // ── Discovery ──────────────────────────────────────

  async runDiscoveryScan() {
    return this.callTool("discovery_scan");
  }

  // ── Database ───────────────────────────────────────

  async checkIntegrity() {
    return this.callTool("db_integrity");
  }

  async backup(path: string) {
    return this.callTool("db_backup", { path });
  }
}
