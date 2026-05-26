import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { load } from "@tauri-apps/plugin-store";
import { Button } from "./button";
import { Input } from "./input";
import { Label } from "./label";
import { Checkbox } from "./checkbox";
import { Loader2, Check } from "lucide-react";
import { cn } from "@/lib/utils";
import { FIXED_AI_MODEL, FIXED_AI_MODEL_LABEL } from "@/lib/ai-config";

interface AppSecretsSettings {
  openrouter_api_key: string;
  openai_api_key: string;
  telegram_bot_token: string;
  telegram_chat_id: string;
}

export function SettingsPage() {
  const [botToken, setBotToken] = useState("");
  const [chatId, setChatId] = useState("");
  const [groqKey, setGroqKey] = useState("");
  const [openaiKey, setOpenaiKey] = useState("");
  const [enableQueries, setEnableQueries] = useState(false);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [saved, setSaved] = useState(false);
  const [loading, setLoading] = useState(true);
  const [activeTab, setActiveTab] = useState<"bot" | "ai">("bot");

  const [keysLoadedFromSupabase, setKeysLoadedFromSupabase] = useState(false);

  useEffect(() => {
    async function loadSettings() {
      try {
        const store = await load("settings.json");
        const queriesEnabled = await store.get<boolean>("telegram_enable_queries");
        if (queriesEnabled !== null && queriesEnabled !== undefined) {
          setEnableQueries(queriesEnabled);
        }

        const remote = await invoke<AppSecretsSettings>("load_app_secrets_settings");
        setGroqKey(remote.openrouter_api_key ?? "");
        setOpenaiKey(remote.openai_api_key ?? "");
        setBotToken(remote.telegram_bot_token ?? "");
        setChatId(remote.telegram_chat_id ?? "");
        setKeysLoadedFromSupabase(
          Boolean(remote.openrouter_api_key?.trim() || remote.openai_api_key?.trim())
        );
      } catch (err) {
        console.error("Failed to load settings:", err);
        setKeysLoadedFromSupabase(false);
      } finally {
        setLoading(false);
      }
    }

    loadSettings();
  }, []);

  const handleSave = async () => {
    setSaving(true);
    try {
      await invoke("save_telegram_settings_local", {
        botToken,
        chatId,
        enableQueries,
      });

      if (groqKey || openaiKey) {
        await invoke("save_app_secrets_settings", {
          settings: {
            openrouter_api_key: groqKey,
            openai_api_key: openaiKey,
            telegram_bot_token: "",
            telegram_chat_id: "",
          } satisfies AppSecretsSettings,
        });
      }

      await invoke("update_telegram_settings").catch(console.error);

      setSaved(true);
      setTimeout(() => setSaved(false), 3000);
    } catch (err) {
      console.error("Failed to save settings:", err);
      alert("حدث خطأ أثناء حفظ الإعدادات: " + err);
    } finally {
      setSaving(false);
    }
  };

  const handleTest = async () => {
    if (!botToken || !chatId) {
      alert("يرجى إدخال توكن البوت ومعرف الدردشة أولاً");
      return;
    }
    setTesting(true);
    try {
      const msg = await invoke<string>("test_telegram_bot", { token: botToken, chatId: chatId });
      alert(msg);
    } catch (err) {
      console.error(err);
      alert("فشل الإرسال: " + err);
    } finally {
      setTesting(false);
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center py-16 text-muted-foreground">
        <Loader2 className="w-5 h-5 animate-spin" />
      </div>
    );
  }

  return (
    <div className="p-6 space-y-6" dir="rtl">
      <div className="flex items-center justify-between pt-4">
        <div>
          <h1 className="text-2xl font-bold">الإعدادات</h1>
          <p className="text-sm text-muted-foreground mt-0.5">
            مفاتيح AI في Supabase — تليجرام محلي لكل مستخدم
          </p>
        </div>
      </div>

      <div className="bg-card border border-border rounded-xl p-5 space-y-6 min-h-[400px] flex flex-col">
        <div className="flex items-center gap-6 border-b border-border">
          <button
            onClick={() => setActiveTab("bot")}
            className={cn(
              "pb-3 text-sm font-semibold transition-colors border-b-2 flex items-center gap-2",
              activeTab === "bot" ? "border-primary text-primary" : "border-transparent text-muted-foreground hover:text-foreground"
            )}
          >
            <img src="/telegram.svg" alt="Telegram" className="w-4 h-4" />
            البوت (Telegram)
          </button>
          <button
            onClick={() => setActiveTab("ai")}
            className={cn(
              "pb-3 text-sm font-semibold transition-colors border-b-2 flex items-center gap-2",
              activeTab === "ai" ? "border-primary text-primary" : "border-transparent text-muted-foreground hover:text-foreground"
            )}
          >
            <img src="/ai.svg" alt="AI" className="w-4 h-4" />
            الذكاء الاصطناعي (OpenRouter)
          </button>
        </div>

        <div className="flex-1">
          {activeTab === "bot" && (
            <div className="space-y-6 animate-in fade-in duration-300">
              <div className="space-y-4">
                <div className="space-y-2">
                  <Label htmlFor="botToken">توكن البوت (Bot Token)</Label>
                  <Input
                    id="botToken"
                    type="password"
                    placeholder="123456789:ABCdefGHIjklMNOpqrsTUVwxyz..."
                    value={botToken}
                    onChange={(e) => setBotToken(e.target.value)}
                  />
                  <p className="text-xs text-muted-foreground">يُحفظ على جهازك فقط — كل مستخدم يربط بوته الخاص</p>
                </div>

                <div className="space-y-2">
                  <Label htmlFor="chatId">معرف الدردشة (Chat ID)</Label>
                  <Input
                    id="chatId"
                    placeholder="مثال: 123456789"
                    value={chatId}
                    onChange={(e) => setChatId(e.target.value)}
                  />
                  <p className="text-xs text-muted-foreground">معرف المستخدم أو المجموعة لاستقبال التقارير</p>
                </div>

                <div className="flex items-center space-x-2 space-x-reverse pt-2">
                  <Checkbox
                    id="enableQueries"
                    checked={enableQueries}
                    onCheckedChange={(c) => setEnableQueries(c as boolean)}
                  />
                  <Label htmlFor="enableQueries" className="font-medium cursor-pointer">
                    تفعيل الاستعلامات عبر البوت
                  </Label>
                </div>
                <p className="text-xs text-muted-foreground mr-6">
                  عند التفعيل، يمكن طلب التقارير من البوت واستلامها كملف Excel/CSV أو PDF.
                </p>
              </div>
            </div>
          )}

          {activeTab === "ai" && (
            <div className="space-y-6 animate-in fade-in duration-300">
              <h4 className="text-sm font-semibold mb-2">إعدادات الذكاء الاصطناعي (OpenRouter)</h4>
              <p className="text-sm text-muted-foreground mb-4">
                مفاتيح OpenRouter و OpenAI مُخزَّنة مركزياً في Supabase — تُحمَّل تلقائياً.
              </p>
              {keysLoadedFromSupabase && (
                <p className="text-sm text-green-600 dark:text-green-400 mb-4">
                  تم تحميل المفاتيح من Supabase — لا حاجة لإدخالها يدوياً.
                </p>
              )}

              <div className="grid gap-2">
                <Label htmlFor="groqKey">مفتاح API (OpenRouter)</Label>
                <Input
                  id="groqKey"
                  type="password"
                  placeholder={keysLoadedFromSupabase ? "محمّل من Supabase" : "sk-or-v1-..."}
                  value={groqKey}
                  onChange={(e) => setGroqKey(e.target.value)}
                  readOnly={keysLoadedFromSupabase}
                />
              </div>

              <div className="grid gap-2">
                <Label htmlFor="openaiKey">مفتاح OpenAI (RAG)</Label>
                <Input
                  id="openaiKey"
                  type="password"
                  placeholder={keysLoadedFromSupabase ? "محمّل من Supabase" : "sk-proj-..."}
                  value={openaiKey}
                  onChange={(e) => setOpenaiKey(e.target.value)}
                  readOnly={keysLoadedFromSupabase}
                />
              </div>

              <div className="grid gap-2">
                <Label>النموذج المستخدم (ثابت)</Label>
                <div
                  className="flex h-10 w-full items-center rounded-md border border-border bg-muted/40 px-3 py-2 text-sm text-muted-foreground"
                  dir="ltr"
                >
                  {FIXED_AI_MODEL}
                </div>
                <p className="text-xs text-muted-foreground">{FIXED_AI_MODEL_LABEL}</p>
              </div>
            </div>
          )}
        </div>

        <div className="pt-4 flex justify-end gap-3">
          <Button variant="outline" onClick={handleTest} disabled={testing || !botToken || !chatId}>
            {testing ? <Loader2 className="w-4 h-4 animate-spin ml-2" /> : null}
            اختبار الإرسال
          </Button>
          <Button onClick={handleSave} disabled={saving}>
            {saving ? <Loader2 className="w-4 h-4 animate-spin ml-2" /> : null}
            {saved ? <Check className="w-4 h-4 ml-2 text-green-500" /> : null}
            {saved ? "تم الحفظ" : "حفظ الإعدادات"}
          </Button>
        </div>
      </div>
    </div>
  );
}
