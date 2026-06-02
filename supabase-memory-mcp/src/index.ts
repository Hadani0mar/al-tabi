#!/usr/bin/env node
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
import axios from "axios";
import { createHash } from "crypto";

// ── إعدادات الافتراضية ───────────────────────────────────────────────
const DEFAULT_SUPABASE_URL = "https://nsgmhijtaaenpqxxgjds.supabase.co";
const DEFAULT_SUPABASE_ANON_KEY = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6Im5zZ21oaWp0YWFlbnBxeHhnamRzIiwicm9sZSI6ImFub24iLCJpYXQiOjE3NzkxODU1NTMsImV4cCI6MjA5NDc2MTU1M30.bva5PiwsoBiLR7u2upQV7q2spl6GhAg-JqrQ8nnUC8E";

// جلب الإعدادات من بيئة العمل أو استخدام الافتراضية
const SUPABASE_URL = process.env.SUPABASE_URL || DEFAULT_SUPABASE_URL;
const SUPABASE_ANON_KEY = process.env.SUPABASE_ANON_KEY || DEFAULT_SUPABASE_ANON_KEY;

// حساب بصمة النص لمنع التكرار (SHA-256)
function contentFingerprint(text: string): string {
  const normalized = text.split(/\s+/).join(" ").toLowerCase().trim();
  return createHash("sha256").update(normalized).digest("hex").substring(0, 16);
}

// توليد Embedding باستخدام OpenAI
async function getEmbedding(text: string, openaiKey: string): Promise<number[]> {
  const key = (openaiKey || process.env.OPENAI_API_KEY || "").trim();
  if (!key) {
    throw new Error("Missing OpenAI API Key. Provide it in the tool argument or via OPENAI_API_KEY environment variable.");
  }
  
  const response = await axios.post(
    "https://api.openai.com/v1/embeddings",
    {
      model: "text-embedding-3-small",
      input: text.substring(0, 8000),
    },
    {
      headers: {
        "Authorization": `Bearer ${key}`,
        "Content-Type": "application/json",
      },
      timeout: 30000,
    }
  );

  const embedding = response.data?.data?.[0]?.embedding;
  if (!embedding || embedding.length !== 1536) {
    throw new Error(`Failed to generate 1536-dim embedding. Response: ${JSON.stringify(response.data)}`);
  }
  return embedding;
}

// استدعاء RPC في Supabase
async function callSupabaseRpc(fnName: string, payload: any): Promise<any> {
  const url = `${SUPABASE_URL}/rest/v1/rpc/${fnName}`;
  const response = await axios.post(url, payload, {
    headers: {
      "apikey": SUPABASE_ANON_KEY,
      "Authorization": `Bearer ${SUPABASE_ANON_KEY}`,
      "Content-Type": "application/json",
    },
    timeout: 30000,
  });
  return response.data;
}

// إنشاء خادم MCP
const server = new McpServer({
  name: "supabase-memory-mcp",
  version: "1.0.0",
});

// ── 1. أداة حفظ حقيقة هيكلية عامة مشتركة ────────────────────────────
server.registerTool(
  "supabase_store_shared_fact",
  {
    title: "Store Shared Database Fact",
    description: "Saves a public, verified technical database schema fact (tables, views, columns, joins) to the shared db_facts table on Supabase. This will be available to all users. DO NOT save any private user data, transactions, or sensitive values.",
    inputSchema: z.object({
      content: z.string().min(5).describe("Technical schema fact to store. Must refer to tables/columns."),
      category: z.string().default("db_schema").describe("Category of the fact: 'db_schema', 'db_join', or 'db_column'"),
      openai_key: z.string().optional().describe("OpenAI API key. If not provided, the OPENAI_API_KEY environment variable will be used."),
    }).strict(),
    annotations: {
      readOnlyHint: false,
      destructiveHint: false,
      idempotentHint: true,
      openWorldHint: false,
    },
  },
  async (params) => {
    try {
      const embedding = await getEmbedding(params.content, params.openai_key || "");
      const fingerprint = contentFingerprint(params.content);

      const result = await callSupabaseRpc("upsert_db_fact", {
        p_content: params.content,
        p_category: params.category,
        p_fingerprint: fingerprint,
        p_embedding: embedding,
      });

      return {
        content: [{ type: "text", text: `Successfully stored public shared fact. ID: ${result}` }],
        structuredContent: { success: true, id: result },
      };
    } catch (error: any) {
      return {
        content: [{ type: "text", text: `Error storing shared fact: ${error.message || error}` }],
      };
    }
  }
);

// ── 2. أداة استرجاع الحقائق العامة المشتركة ─────────────────────────
server.registerTool(
  "supabase_retrieve_shared_facts",
  {
    title: "Retrieve Shared Database Facts",
    description: "Queries the public shared db_facts table on Supabase using vector similarity search to find relevant database schema structures and rules.",
    inputSchema: z.object({
      query: z.string().min(3).describe("Search query to match against database schema facts."),
      limit: z.number().int().min(1).max(20).default(4).describe("Maximum number of records to return."),
      openai_key: z.string().optional().describe("OpenAI API key."),
    }).strict(),
    annotations: {
      readOnlyHint: true,
      destructiveHint: false,
      idempotentHint: true,
      openWorldHint: true,
    },
  },
  async (params) => {
    try {
      const embedding = await getEmbedding(params.query, params.openai_key || "");
      const rows = await callSupabaseRpc("match_db_facts", {
        query_embedding: embedding,
        match_threshold: 0.35,
        match_count: params.limit,
      });

      if (!rows || rows.length === 0) {
        return { content: [{ type: "text", text: "No relevant shared schema facts found." }] };
      }

      const formatted = rows
        .map((r: any) => `- [${r.category}] ${r.content} (similarity: ${(r.similarity * 100).toFixed(1)}%)`)
        .join("\n");

      return {
        content: [{ type: "text", text: formatted }],
        structuredContent: { facts: rows },
      };
    } catch (error: any) {
      return {
        content: [{ type: "text", text: `Error retrieving shared facts: ${error.message || error}` }],
      };
    }
  }
);

// ── 3. أداة حفظ تفضيل أو ذاكرة خاصة لمستخدم معزول بالتوكن ───────────
server.registerTool(
  "supabase_store_private_memory",
  {
    title: "Store Private User Memory",
    description: "Saves a private preference or user-specific preference to the user_memories table on Supabase, isolated by SHA-256 token hashing. Other users will not see this.",
    inputSchema: z.object({
      access_token: z.string().min(16).describe("The user's app access token to isolate their memory space."),
      content: z.string().min(3).describe("Private user preference to store (e.g., 'User prefers a 30-day default window')."),
      category: z.string().default("preference").describe("Category of the memory (e.g. 'preference', 'chat_note')."),
      openai_key: z.string().optional().describe("OpenAI API key."),
    }).strict(),
    annotations: {
      readOnlyHint: false,
      destructiveHint: false,
      idempotentHint: true,
      openWorldHint: false,
    },
  },
  async (params) => {
    try {
      const embedding = await getEmbedding(params.content, params.openai_key || "");
      const fingerprint = contentFingerprint(params.content);

      const result = await callSupabaseRpc("upsert_user_memory", {
        p_access_token: params.access_token,
        p_content: params.content,
        p_category: params.category,
        p_fingerprint: fingerprint,
        p_embedding: embedding,
      });

      return {
        content: [{ type: "text", text: `Successfully stored private user preference. ID: ${result}` }],
        structuredContent: { success: true, id: result },
      };
    } catch (error: any) {
      return {
        content: [{ type: "text", text: `Error storing private memory: ${error.message || error}` }],
      };
    }
  }
);

// ── 4. أداة استرجاع الذاكرة الخاصة بالمستخدم ──────────────────────────
server.registerTool(
  "supabase_retrieve_private_memories",
  {
    title: "Retrieve Private User Memories",
    description: "Queries the private user_memories table on Supabase using vector similarity search, isolated by the user's access token hash. Only returns memories belonging to this user.",
    inputSchema: z.object({
      access_token: z.string().min(16).describe("The user's app access token to retrieve their specific memory space."),
      query: z.string().min(3).describe("Search query to find relevant private memories/preferences."),
      limit: z.number().int().min(1).max(20).default(3).describe("Maximum number of memories to return."),
      openai_key: z.string().optional().describe("OpenAI API key."),
    }).strict(),
    annotations: {
      readOnlyHint: true,
      destructiveHint: false,
      idempotentHint: true,
      openWorldHint: true,
    },
  },
  async (params) => {
    try {
      const embedding = await getEmbedding(params.query, params.openai_key || "");
      const rows = await callSupabaseRpc("match_user_memories", {
        p_access_token: params.access_token,
        query_embedding: embedding,
        match_threshold: 0.35,
        match_count: params.limit,
      });

      if (!rows || rows.length === 0) {
        return { content: [{ type: "text", text: "No private preferences or memories found." }] };
      }

      const formatted = rows
        .map((r: any) => `- [${r.category}] ${r.content} (similarity: ${(r.similarity * 100).toFixed(1)}%)`)
        .join("\n");

      return {
        content: [{ type: "text", text: formatted }],
        structuredContent: { memories: rows },
      };
    } catch (error: any) {
      return {
        content: [{ type: "text", text: `Error retrieving private memories: ${error.message || error}` }],
      };
    }
  }
);

// ── 5. أداة مسح الذاكرة الخاصة بالكامل للمستخدم ─────────────────────────
server.registerTool(
  "supabase_clear_private_memories",
  {
    title: "Clear Private User Memories",
    description: "Deletes all private memories and preferences stored on Supabase for this user, identified by their access token.",
    inputSchema: z.object({
      access_token: z.string().min(16).describe("The user's app access token to delete their memory space."),
    }).strict(),
    annotations: {
      readOnlyHint: false,
      destructiveHint: true,
      idempotentHint: true,
      openWorldHint: false,
    },
  },
  async (params) => {
    try {
      await callSupabaseRpc("clear_user_memories", {
        p_access_token: params.access_token,
      });

      return {
        content: [{ type: "text", text: "Successfully cleared all your private preferences and memories from Supabase." }],
        structuredContent: { success: true },
      };
    } catch (error: any) {
      return {
        content: [{ type: "text", text: `Error clearing private memories: ${error.message || error}` }],
      };
    }
  }
);

// ── التشغيل عبر Stdio ────────────────────────────────────────────────
async function main() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
  console.error("Supabase Memory MCP Server running via stdio");
}

main().catch((error) => {
  console.error("Server fatal error:", error);
  process.exit(1);
});
