# synapsis-client

TypeScript/JavaScript SDK for [Synapsis](https://github.com/MethodWhite/synapsis) MCP Server.

## Install

```bash
npm install synapsis-client
```

## Usage

```typescript
import { SynapsisClient } from "synapsis-client";

const synapsis = new SynapsisClient();

// Save memory
await synapsis.saveMemory("API design decision", "Use MCP protocol for all agent communication", {
  project: "my-project",
});

// Search
const results = await synapsis.searchMemory("API design");
console.log(results);

// Sessions
const session = await synapsis.startSession("my-project");
await synapsis.endSession(session, "Completed initial setup");

// Discovery
const scan = await synapsis.runDiscoveryScan();
console.log(scan);
```

## API

See [Synapsis MCP Tools](https://github.com/MethodWhite/synapsis#mcp-tools) for the full list.
