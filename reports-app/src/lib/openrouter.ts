/**
 * OpenRouter client using the official OpenAI SDK with a custom base URL.
 * يدعم streaming، tool calling، وجميع مزايا OpenRouter عبر OpenAI-compatible API.
 */
import OpenAI from "openai";

const OPENROUTER_BASE_URL = "https://openrouter.ai/api/v1";

export type ChatMessage = OpenAI.Chat.ChatCompletionMessageParam;

/** أنشئ OpenRouter client بمفتاح API */
export function createOpenRouterClient(apiKey: string): OpenAI {
  return new OpenAI({
    apiKey,
    baseURL: OPENROUTER_BASE_URL,
    defaultHeaders: {
      "HTTP-Referer": "http://localhost:1420",
      "X-Title": "Reports App",
    },
    dangerouslyAllowBrowser: true, // Tauri desktop — لا خادم وسيط
  });
}

/**
 * استدعاء بسيط — يرجع النص كاملاً بعد انتهاء الاستجابة.
 * مناسب للعمليات الخفيفة كتوليد العناوين.
 */
export async function quickChat(
  apiKey: string,
  model: string,
  messages: ChatMessage[],
  maxTokens = 100
): Promise<string> {
  const client = createOpenRouterClient(apiKey);
  const response = await client.chat.completions.create({
    model,
    messages,
    max_tokens: maxTokens,
  });
  return response.choices[0]?.message?.content ?? "";
}

/**
 * Streaming — يُرسل النص chunk بـ chunk عبر callback.
 * يعرض الرد للمستخدم فوراً بدل الانتظار حتى الانتهاء.
 */
export async function streamChat(
  apiKey: string,
  model: string,
  messages: ChatMessage[],
  onChunk: (delta: string) => void,
  onDone?: (fullText: string) => void,
  maxTokens = 4096
): Promise<string> {
  const client = createOpenRouterClient(apiKey);
  const stream = await client.chat.completions.create({
    model,
    messages,
    max_tokens: maxTokens,
    stream: true,
  });

  let fullText = "";
  for await (const chunk of stream) {
    const delta = chunk.choices[0]?.delta?.content ?? "";
    if (delta) {
      fullText += delta;
      onChunk(delta);
    }
  }
  onDone?.(fullText);
  return fullText;
}

/**
 * توليد عنوان قصير للمحادثة (3 كلمات كحد أقصى).
 * تُستدعى تلقائياً عند بدء محادثة جديدة.
 */
export async function generateChatTitle(
  apiKey: string,
  model: string,
  userMessage: string
): Promise<string> {
  const text = await quickChat(
    apiKey,
    model,
    [
      {
        role: "user",
        content: `لخص هذه الجملة في 3 كلمات كحد أقصى لتكون عنواناً لمحادثة بدون أي مقدمات أو علامات تنصيص: "${userMessage}"`,
      },
    ],
    60
  );
  return text.replace(/["']/g, "").trim();
}
