/** النموذج الثابت للوكيل عبر OpenRouter */
export const FIXED_AI_MODEL = "minimax/minimax-m3";

export const FIXED_AI_MODEL_LABEL = "MiniMax M3 — أدوات SQL + عربية";

/**
 * نماذج Anthropic Claude متاحة عبر OpenRouter — للاستخدام المستقبلي.
 * لتغيير النموذج: عدّل FIXED_AI_MODEL في ai_agent.rs (DEFAULT_AI_MODEL).
 */
export const CLAUDE_MODELS = {
  /** Claude Haiku 4.5 — الأسرع والأرخص للاستعلامات البسيطة */
  HAIKU: "anthropic/claude-haiku-4-5",
  /** Claude Sonnet 4.6 — متوازن (سرعة + جودة) */
  SONNET: "anthropic/claude-sonnet-4-6",
  /** Claude Opus 4.8 — الأقوى للتحليلات المعقدة */
  OPUS: "anthropic/claude-opus-4-8",
} as const;
