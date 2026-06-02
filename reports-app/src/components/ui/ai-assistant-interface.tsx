"use client";

import type React from "react";
import { useState, useRef, useEffect, useCallback } from "react";
import {
  ArrowUp,
  Plus,
  Sparkles,
  Loader2,
  Trash2,
  Download,
  Menu,
  MessageSquare,
  PlusCircle,
  Square,
  Tag,
  ChevronDown,
  ChevronUp,
} from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { load } from "@tauri-apps/plugin-store";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { cn } from "@/lib/utils";

export interface ChatSession {
  id: string;
  title: string;
  messages: Message[];
  updatedAt: number;
}

interface Message {
  role: "user" | "assistant" | "system";
  content: string;
  aiUsage?: AiTokenUsage;
}

interface AiTokenUsage {
  model: string;
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
  usageSource?: string;
  generationId?: string;
  cost?: number;
}

interface AiUsagePayload extends AiTokenUsage {
  requestId: string;
}

interface ProductMention {
  name: string;
  code: string;
}

interface MentionContext {
  start: number;
  query: string;
  caret: number;
}

function formatProductMention(hit: ProductMention): string {
  const name = hit.name.trim();
  const code = hit.code.trim();
  if (code && code !== name) {
    return `@${name} (${code})`;
  }
  return `@${name}`;
}

function getMentionContext(text: string, caret: number): MentionContext | null {
  const before = text.slice(0, caret);
  const at = before.lastIndexOf("@");
  if (at === -1) return null;
  const query = before.slice(at + 1);
  if (/[\s\n\r]/.test(query)) return null;
  return { start: at, query, caret };
}

interface Props {
  groqKey: string;
  aiModel: string;
}

export function AIAssistantInterface({ groqKey, aiModel }: Props) {
  const [inputValue, setInputValue] = useState("");

  const [chatHistory, setChatHistory] = useState<Message[]>([]);
  const [loadingChatIds, setLoadingChatIds] = useState<Set<string>>(new Set());
  const [toolProgress, setToolProgress] = useState<string | null>(null);

  const [chats, setChats] = useState<ChatSession[]>([]);
  const [activeChatId, setActiveChatId] = useState<string | null>(null);
  const [isSidebarOpen, setIsSidebarOpen] = useState(false);
  const [isInputCollapsed, setIsInputCollapsed] = useState(false);
  const [isSending, setIsSending] = useState(false);

  const inputRef = useRef<HTMLTextAreaElement>(null);
  const mentionDropRef = useRef<HTMLDivElement>(null);
  const chatEndRef = useRef<HTMLDivElement>(null);
  const pendingByChatRef = useRef<Record<string, string>>({});
  const loadingChatIdsRef = useRef<Set<string>>(new Set());
  const sendLockRef = useRef(false);
  const activeChatIdRef = useRef<string | null>(null);
  const mentionDebounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const usageByRequestRef = useRef<Record<string, AiTokenUsage>>({});

  const [productSuggestions, setProductSuggestions] = useState<ProductMention[]>([]);
  const [mentionLoading, setMentionLoading] = useState(false);
  const [showMentionDrop, setShowMentionDrop] = useState(false);
  const [mentionFocusIdx, setMentionFocusIdx] = useState(-1);
  const [mentionCtx, setMentionCtx] = useState<MentionContext | null>(null);

  const isActiveChatLoading =
    isSending ||
    (activeChatIdRef.current !== null &&
      loadingChatIds.has(activeChatIdRef.current)) ||
    (activeChatId !== null && loadingChatIds.has(activeChatId));

  const saveLastActiveChatId = async (id: string | null) => {
    try {
      const store = await load("chats.json");
      if (id) {
        await store.set("lastActiveChatId", id);
      } else {
        await store.delete("lastActiveChatId");
      }
      await store.save();
    } catch (e) {
      console.error("Failed to save active chat id:", e);
    }
  };

  useEffect(() => {
    async function loadChats() {
      try {
        let supabaseChats: ChatSession[] | null = null;
        try {
          const remoteChats = await invoke<any[]>("fetch_chats_from_supabase");
          if (remoteChats && Array.isArray(remoteChats)) {
            supabaseChats = remoteChats.map((c: any) => ({
              id: c.chat_id,
              title: c.title,
              messages: c.messages,
              updatedAt: new Date(c.updated_at).getTime(),
            }));
          }
        } catch (err) {
          console.warn("Failed to fetch chats from Supabase, falling back to local:", err);
        }

        const store = await load("chats.json");
        const lastId = await store.get<string>("lastActiveChatId");
        
        let finalChats: ChatSession[] = [];
        if (supabaseChats?.length) {
          finalChats = supabaseChats;
          await store.set("history", supabaseChats);
          await store.save();
        } else {
          const savedChats = await store.get<ChatSession[]>("history");
          if (savedChats?.length) {
            finalChats = savedChats;
          }
        }

        if (finalChats.length) {
          setChats(finalChats);
          if (lastId) {
            const last = finalChats.find((c) => c.id === lastId);
            if (last) {
              activeChatIdRef.current = last.id;
              setActiveChatId(last.id);
              setChatHistory(last.messages);
            }
          }
        }
      } catch (e) {
        console.error("Failed to load chats:", e);
      }
    }
    loadChats();
  }, []);

  // Scroll to bottom when new messages arrive
  useEffect(() => {
    chatEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [chatHistory, loadingChatIds, toolProgress, activeChatId]);

  useEffect(() => {
    const unlisten = listen<{tool: string}>("tool-usage", (event) => {
        setToolProgress(event.payload.tool);
    });
    return () => {
        unlisten.then(f => f());
    };
  }, []);

  useEffect(() => {
    const unlisten = listen<AiUsagePayload>("ai-usage", (event) => {
      const payload = event.payload;
      if (!payload.requestId) return;
      const prev = usageByRequestRef.current[payload.requestId];
      usageByRequestRef.current[payload.requestId] = {
        model: payload.model || prev?.model || aiModel,
        promptTokens: (prev?.promptTokens ?? 0) + (payload.promptTokens ?? 0),
        completionTokens: (prev?.completionTokens ?? 0) + (payload.completionTokens ?? 0),
        totalTokens: (prev?.totalTokens ?? 0) + (payload.totalTokens ?? 0),
        usageSource: payload.usageSource || prev?.usageSource,
        generationId: payload.generationId || prev?.generationId,
        cost: (prev?.cost ?? 0) + (payload.cost ?? 0),
      };
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, [aiModel]);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (
        mentionDropRef.current &&
        !mentionDropRef.current.contains(e.target as Node) &&
        inputRef.current &&
        !inputRef.current.contains(e.target as Node)
      ) {
        setShowMentionDrop(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  const fetchProductMentions = useCallback(async (q: string) => {
    setMentionLoading(true);
    try {
      const hits = await invoke<ProductMention[]>("search_product_mentions", { query: q });
      setProductSuggestions(hits);
      setShowMentionDrop(hits.length > 0 || q.length === 0);
      setMentionFocusIdx(-1);
    } catch (e) {
      console.error("search_product_mentions:", e);
      setProductSuggestions([]);
      setShowMentionDrop(false);
    } finally {
      setMentionLoading(false);
    }
  }, []);

  const syncMentionFromInput = useCallback(
    (text: string, caret: number) => {
      const ctx = getMentionContext(text, caret);
      setMentionCtx(ctx);
      if (!ctx) {
        setShowMentionDrop(false);
        setProductSuggestions([]);
        if (mentionDebounceRef.current) clearTimeout(mentionDebounceRef.current);
        return;
      }
      setShowMentionDrop(true);
      if (mentionDebounceRef.current) clearTimeout(mentionDebounceRef.current);
      mentionDebounceRef.current = setTimeout(() => {
        fetchProductMentions(ctx.query);
      }, 200);
    },
    [fetchProductMentions]
  );

  const selectProductMention = (hit: ProductMention) => {
    if (!mentionCtx || !inputRef.current) return;
    const token = formatProductMention(hit);
    const before = inputValue.slice(0, mentionCtx.start);
    const after = inputValue.slice(mentionCtx.caret);
    const next = `${before}${token} ${after}`;
    const caret = before.length + token.length + 1;
    setInputValue(next);
    setShowMentionDrop(false);
    setMentionCtx(null);
    setProductSuggestions([]);
    requestAnimationFrame(() => {
      const el = inputRef.current;
      if (el) {
        el.focus();
        el.setSelectionRange(caret, caret);
      }
    });
  };

  const handleInputChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const val = e.target.value;
    const caret = e.target.selectionStart ?? val.length;
    setInputValue(val);
    syncMentionFromInput(val, caret);
  };

  const handleInputSelect = (e: React.SyntheticEvent<HTMLTextAreaElement>) => {
    const el = e.currentTarget;
    syncMentionFromInput(el.value, el.selectionStart ?? el.value.length);
  };

  const handleInputKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (showMentionDrop && productSuggestions.length > 0) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setMentionFocusIdx((i) => Math.min(i + 1, productSuggestions.length - 1));
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setMentionFocusIdx((i) => Math.max(i - 1, 0));
        return;
      }
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        const idx = mentionFocusIdx >= 0 ? mentionFocusIdx : 0;
        selectProductMention(productSuggestions[idx]);
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        setShowMentionDrop(false);
        return;
      }
    }
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSendMessage();
    }
  };

  const quickPrompts = [
    "ما مبيعات اليوم؟",
    "اعرض ديون العملاء",
    "اعرض ديون الموظفين",
    "ما المنتجات المنقطعة؟",
    "من هم أفضل 10 عملاء؟",
    "منتجات قاربت على الانتهاء",
  ];

  const handleQuickPrompt = (prompt: string) => {
    setInputValue(prompt);
    inputRef.current?.focus();
  };

  const saveChatsToStore = async (newChats: ChatSession[], chatIdToSync?: string) => {
    try {
      const store = await load("chats.json");
      await store.set("history", newChats);
      await store.save();

      const syncId = chatIdToSync || activeChatIdRef.current || activeChatId;
      if (syncId) {
        const chat = newChats.find(c => c.id === syncId);
        if (chat) {
          invoke("sync_chat_to_supabase", {
            chatId: chat.id,
            title: chat.title,
            messages: chat.messages,
          }).catch(err => console.error("Supabase chat sync failed:", err));
        }
      }
    } catch (e) {
      console.error("Failed to save chats:", e);
    }
  };

  const handleNewChat = () => {
    setChatHistory([]);
    activeChatIdRef.current = null;
    setActiveChatId(null);
    setIsSidebarOpen(false);
    saveLastActiveChatId(null);
  };

  const clearChatLoading = (chatId: string) => {
    loadingChatIdsRef.current.delete(chatId);
    setLoadingChatIds((prev) => {
      const next = new Set(prev);
      next.delete(chatId);
      return next;
    });
  };

  const releaseChatRequest = (chatId: string, requestId: string) => {
    if (pendingByChatRef.current[chatId] === requestId) {
      delete pendingByChatRef.current[chatId];
    }
    clearChatLoading(chatId);
    if (loadingChatIdsRef.current.size === 0) {
      sendLockRef.current = false;
      setIsSending(false);
      setToolProgress(null);
    }
  };

  const syncVisibleHistory = (chatId: string, history: Message[]) => {
    const viewingId = activeChatIdRef.current ?? activeChatId;
    if (viewingId === chatId) {
      setChatHistory(history);
    }
  };

  const handleStopMessage = async () => {
    const chatId = activeChatIdRef.current ?? activeChatId;
    if (!chatId) return;
    const requestId = pendingByChatRef.current[chatId];
    if (!requestId) return;
    try {
      await invoke("cancel_local_ai", { requestId });
    } catch (e) {
      console.error("cancel failed:", e);
    }
    clearChatLoading(chatId);
    delete pendingByChatRef.current[chatId];
    sendLockRef.current = false;
    setIsSending(false);
    setToolProgress(null);
    const stopMsg: Message = {
      role: "assistant",
      content: "⏹ تم إيقاف الرد. يمكنك إرسال رسالة جديدة.",
    };
    setChatHistory((hist) => {
      const updated = [...hist, stopMsg];
      setChats((prev) => {
        const newC = prev.map((c) =>
          c.id === chatId
            ? { ...c, messages: updated, updatedAt: Date.now() }
            : c
        );
        saveChatsToStore(newC);
        return newC;
      });
      return updated;
    });
  };

  const handleSendMessage = async () => {
    const trimmed = inputValue.trim();
    if (!trimmed || sendLockRef.current) return;

    let currentChatId = activeChatIdRef.current ?? activeChatId;
    let isNewChat = false;
    if (!currentChatId) {
      currentChatId = crypto.randomUUID();
      isNewChat = true;
    }

    if (loadingChatIdsRef.current.has(currentChatId)) return;

    sendLockRef.current = true;
    setIsSending(true);
    const userMessage = trimmed;
    setInputValue("");
    setShowMentionDrop(false);
    setMentionCtx(null);
    const newMsg: Message = { role: "user", content: userMessage };
    const newHistory = [...chatHistory, newMsg];
    setChatHistory(newHistory);
    setToolProgress(null);

    activeChatIdRef.current = currentChatId;
    if (isNewChat) {
      setActiveChatId(currentChatId);
      saveLastActiveChatId(currentChatId);
    }

    const requestId = crypto.randomUUID();
    pendingByChatRef.current[currentChatId] = requestId;
    loadingChatIdsRef.current.add(currentChatId);
    setLoadingChatIds((prev) => new Set(prev).add(currentChatId));
    
    let updatedChats = [...chats];
    if (isNewChat) {
      updatedChats.unshift({
        id: currentChatId,
        title: "محادثة جديدة",
        messages: newHistory,
        updatedAt: Date.now()
      });
    } else {
      updatedChats = updatedChats.map(c => c.id === currentChatId ? { ...c, messages: newHistory, updatedAt: Date.now() } : c);
    }
    setChats(updatedChats);
    saveChatsToStore(updatedChats);

    if (isNewChat && groqKey.trim()) {
      fetch("https://openrouter.ai/api/v1/chat/completions", {
        method: "POST",
        headers: {
          "Authorization": `Bearer ${groqKey.trim()}`,
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          model: aiModel,
          messages: [{ role: "user", content: `لخص هذه الجملة في 3 كلمات كحد أقصى لتكون عنواناً لمحادثة بدون أي مقدمات أو علامات تنصيص: "${userMessage}"` }]
        })
      }).then(res => res.json()).then(data => {
        if (data.choices && data.choices[0]) {
          const title = data.choices[0].message.content.replace(/["']/g, "").trim();
          setChats(prev => {
            const newC = prev.map(c => c.id === currentChatId ? { ...c, title } : c);
            saveChatsToStore(newC);
            return newC;
          });
        }
      }).catch(console.error);
    }

    const historyForApi = chatHistory;

    try {
        const response = await invoke<string>("ask_local_ai", {
            message: userMessage,
            history: historyForApi,
            groqKey: groqKey,
            aiModel: aiModel,
            requestId,
        });

        if (pendingByChatRef.current[currentChatId] !== requestId) {
          console.warn("[AI] ignored stale response", { currentChatId, requestId });
          return;
        }

        const text = (response ?? "").trim();
        if (!text) {
          throw new Error("رد فارغ من الوكيل — أعد المحاولة.");
        }

        const usage = usageByRequestRef.current[requestId];
        delete usageByRequestRef.current[requestId];
        const assistMsg: Message = { role: "assistant", content: text, aiUsage: usage };
        const finalHistory = [...newHistory, assistMsg];
        setChats((prev) => {
           const newC = prev.map((c) =>
             c.id === currentChatId
               ? { ...c, messages: finalHistory, updatedAt: Date.now() }
               : c
           );
           saveChatsToStore(newC);
           return newC;
        });
        syncVisibleHistory(currentChatId, finalHistory);
    } catch (e) {
        if (pendingByChatRef.current[currentChatId] !== requestId) return;
        console.error(e);
        const errText = String(e);
        const usage = usageByRequestRef.current[requestId];
        delete usageByRequestRef.current[requestId];
        const errMsg: Message = {
          role: "assistant",
          content: errText.includes("إيقاف")
            ? "⏹ تم إيقاف الرد. يمكنك إرسال رسالة جديدة."
            : `❌ عذراً، حدث خطأ: ${errText}`,
        };
        errMsg.aiUsage = usage;
        const finalHistory = [...newHistory, errMsg];
        setChats((prev) => {
           const newC = prev.map((c) =>
             c.id === currentChatId
               ? { ...c, messages: finalHistory, updatedAt: Date.now() }
               : c
           );
           saveChatsToStore(newC);
           return newC;
        });
        syncVisibleHistory(currentChatId, finalHistory);
    } finally {
        releaseChatRequest(currentChatId, requestId);
    }
  };

  const selectChat = (id: string) => {
    const chat = chats.find((c) => c.id === id);
    if (chat) {
      setChatHistory(chat.messages);
      activeChatIdRef.current = chat.id;
      setActiveChatId(chat.id);
      saveLastActiveChatId(chat.id);
      setIsSidebarOpen(false);
    }
  };

  const deleteChat = (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    const newChats = chats.filter(c => c.id !== id);
    setChats(newChats);
    saveChatsToStore(newChats, id);
    invoke("delete_chat_from_supabase", { chatId: id }).catch(err =>
      console.error("Failed to delete chat from Supabase:", err)
    );
    if (activeChatId === id) {
      handleNewChat();
    }
  };

  return (
    <div className="flex w-full min-h-[calc(100vh-6rem)] bg-background relative overflow-x-hidden" dir="rtl">
      
      {/* Sidebar Drawer */}
      <AnimatePresence>
        {isSidebarOpen && (
          <>
            <motion.div 
              initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}
              className="absolute inset-0 z-40 bg-black/50" 
              onClick={() => setIsSidebarOpen(false)} 
            />
            <motion.div 
              initial={{ x: "100%" }} animate={{ x: 0 }} exit={{ x: "100%" }} transition={{ type: "spring", bounce: 0, duration: 0.3 }}
              className="absolute right-0 top-0 bottom-0 w-72 bg-card border-l z-50 flex flex-col shadow-2xl"
            >
              <div className="p-4 border-b flex items-center justify-between">
                <h2 className="font-semibold text-lg flex items-center gap-2"><MessageSquare className="w-5 h-5 text-primary"/> سجل المحادثات</h2>
                <button onClick={handleNewChat} className="p-2 bg-primary text-primary-foreground rounded-md hover:opacity-90 transition-opacity">
                  <PlusCircle className="w-5 h-5" />
                </button>
              </div>
              <div className="flex-1 overflow-y-auto p-2 space-y-2">
                {chats.length === 0 ? (
                  <p className="text-center text-muted-foreground text-sm mt-10">لا توجد محادثات سابقة</p>
                ) : (
                  chats.map(chat => (
                    <div 
                      key={chat.id} 
                      onClick={() => selectChat(chat.id)}
                      className={`flex items-center justify-between p-3 rounded-lg cursor-pointer transition-colors group ${activeChatId === chat.id ? 'bg-primary/10 border-primary/20 border' : 'hover:bg-muted border border-transparent'}`}
                    >
                      <div className="flex flex-col overflow-hidden">
                        <span className="truncate font-medium text-sm flex items-center gap-1.5">
                          {loadingChatIds.has(chat.id) && (
                            <Loader2 className="w-3 h-3 animate-spin text-primary shrink-0" />
                          )}
                          {chat.title}
                        </span>
                        <span className="text-xs text-muted-foreground mt-1">{new Date(chat.updatedAt).toLocaleDateString()}</span>
                      </div>
                      <button onClick={(e) => deleteChat(chat.id, e)} className="p-1.5 text-muted-foreground hover:text-destructive hover:bg-destructive/10 rounded-md transition-colors opacity-0 group-hover:opacity-100 lg:opacity-100">
                        <Trash2 className="w-4 h-4" />
                      </button>
                    </div>
                  ))
                )}
              </div>
            </motion.div>
          </>
        )}
      </AnimatePresence>

      <div className="flex-1 flex flex-col items-center p-4 h-[calc(100vh-6rem)] relative overflow-visible min-h-0">
        {/* Hamburger Menu Header */}
        <div className="w-full mx-auto flex items-center justify-between mb-4 px-2">
          <button onClick={() => setIsSidebarOpen(true)} className="p-2 hover:bg-muted rounded-md border shadow-sm bg-card transition-colors flex items-center gap-2">
            <Menu className="w-5 h-5" />
            <span className="text-sm font-medium">سجل المحادثات</span>
          </button>
          <div
            className="hidden sm:flex items-center gap-2 rounded-md border px-3 py-2 text-xs shadow-sm bg-card"
            style={{ color: "var(--fg-2)", borderColor: "var(--border-subtle)" }}
            dir="ltr"
          >
            <Sparkles className="w-3.5 h-3.5" style={{ color: "var(--brand-accent)" }} />
            <span className="font-mono">{aiModel}</span>
          </div>
          {activeChatId && (
            <button onClick={handleNewChat} className="p-2 hover:bg-muted rounded-md border shadow-sm bg-card transition-colors flex items-center gap-2">
              <Plus className="w-5 h-5" />
              <span className="text-sm font-medium hidden sm:inline">محادثة جديدة</span>
            </button>
          )}
        </div>

        <div className="w-full mx-auto flex flex-col h-[calc(100vh-12rem)] min-h-0 overflow-visible">
        
        {/* Header / Logo */}
        {chatHistory.length === 0 && (
          <div className="flex flex-col items-center justify-center flex-1 animate-in fade-in zoom-in duration-500">
            <div className="mb-6 w-14 h-14 rounded-[14px] flex items-center justify-center" style={{ background: "var(--brand-accent-soft)", color: "var(--brand-accent)" }}>
               <Sparkles className="w-7 h-7" />
            </div>
            <div className="mb-8 text-center">
              <motion.div
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.3 }}
                className="flex flex-col items-center"
              >
                <h1 className="text-3xl font-bold mb-2" style={{ fontFamily: "var(--font-display)", color: "var(--fg-1)" }}>
                  اسأل عن بياناتك بالعربية
                </h1>
                <p className="text-muted-foreground max-w-md text-sm">
                  اكتب سؤالاً أو اختر اقتراحاً. سيتم تنفيذ استعلام آمن (قراءة فقط) على Marketing2026.
                </p>
              </motion.div>
            </div>

            <div className="w-full max-w-2xl flex flex-wrap justify-center gap-2 mb-6">
              {quickPrompts.map((prompt) => (
                <button
                  key={prompt}
                  type="button"
                  onClick={() => handleQuickPrompt(prompt)}
                  className="inline-flex items-center gap-1.5 rounded-full border border-border bg-card px-3 py-1.5 text-xs text-foreground transition-colors hover:border-primary/40 hover:bg-primary/5"
                >
                  <Sparkles className="w-3.5 h-3.5 text-primary shrink-0" />
                  {prompt}
                </button>
              ))}
            </div>
          </div>
        )}

        {/* Chat History */}
         {chatHistory.length > 0 && (
          <div className="flex-1 overflow-y-auto w-full px-4 py-6 space-y-6 scrollbar-hide">
             {chatHistory.map((msg, i) => {
                let content = msg.content;
                let filePath = null;
                const fileMatch = content.match(/\[FILE_PATH:(.*?)\]/);
                if (fileMatch) {
                    filePath = fileMatch[1].trim();
                    content = content.replace(/\[FILE_PATH:.*?\]/g, "");
                }
                
                return (
                <div key={i} className={`flex ${msg.role === 'user' ? 'justify-start' : 'justify-end'}`}>
                   <div
                     className={`max-w-[88%] rounded-2xl px-5 py-4 shadow-sm ${msg.role === 'user' ? 'rounded-br-sm' : 'rounded-bl-sm'}`}
                     style={
                       msg.role === 'user'
                         ? { background: 'var(--user-bubble-bg)', color: 'var(--user-bubble-fg)' }
                         : { background: 'var(--ai-bubble-bg)', color: 'var(--ai-bubble-fg)', border: '1px solid var(--ai-bubble-border)' }
                     }
                   >
                      {msg.role === 'user' ? (
                          <div className="text-[15px] whitespace-pre-wrap leading-relaxed">{content}</div>
                      ) : (
                          <div className="flex flex-col gap-3">
                            <div
                              className="max-w-none text-[15px] leading-relaxed [&_p]:my-2 [&_p]:leading-relaxed [&_ul]:my-2 [&_ol]:my-2 [&_li]:my-0.5 [&_strong]:font-bold [&_h1]:text-lg [&_h2]:text-base [&_h3]:text-[15px] [&_h1]:font-bold [&_h2]:font-bold [&_h3]:font-semibold [&_code]:rounded [&_code]:px-1 [&_code]:py-0.5 [&_code]:text-[13px] [&_code]:font-mono [&_pre]:rounded-xl [&_pre]:border [&_pre]:p-3 [&_pre]:overflow-x-auto [&_pre]:text-[13px] [&_pre]:font-mono [&_table]:w-full [&_th]:px-3 [&_th]:py-2 [&_td]:px-3 [&_td]:py-2"
                              style={{
                                color: "var(--ai-bubble-fg)",
                                ["--tw-prose-body" as string]: "var(--ai-bubble-fg)",
                              }}
                            >
                               <ReactMarkdown
                                  remarkPlugins={[remarkGfm]}
                                  components={{
                                    p: ({ node, ...props }) => <p style={{ color: "var(--ai-bubble-fg)" }} {...props} />,
                                    li: ({ node, ...props }) => <li style={{ color: "var(--ai-bubble-fg)" }} {...props} />,
                                    strong: ({ node, ...props }) => <strong style={{ color: "var(--fg-1)" }} {...props} />,
                                    code: ({ node, ...props }) => (
                                      <code
                                        style={{
                                          background: "var(--bg-muted)",
                                          color: "var(--fg-1)",
                                        }}
                                        {...props}
                                      />
                                    ),
                                    pre: ({ node, ...props }) => (
                                      <pre
                                        style={{
                                          background: "var(--bg-muted)",
                                          borderColor: "var(--border-default)",
                                          color: "var(--fg-1)",
                                        }}
                                        {...props}
                                      />
                                    ),
                                    table: ({ node, ...props }) => (
                                      <div
                                        className="w-full overflow-x-auto my-5 rounded-xl border shadow-sm"
                                        style={{ borderColor: "var(--border-default)", background: "var(--bg-elevated)" }}
                                      >
                                        <table className="w-full text-[15px] text-right border-collapse" {...props} />
                                      </div>
                                    ),
                                    thead: ({ node, ...props }) => (
                                      <thead
                                        className="text-[13px] font-bold uppercase tracking-wide"
                                        style={{ background: "var(--bg-subtle)", color: "var(--fg-1)" }}
                                        {...props}
                                      />
                                    ),
                                    th: ({ node, ...props }) => (
                                      <th
                                        className="px-4 py-3 font-bold border-b-2 whitespace-nowrap text-right"
                                        style={{ borderColor: "var(--border-default)", color: "var(--fg-1)" }}
                                        {...props}
                                      />
                                    ),
                                    td: ({ node, ...props }) => (
                                      <td
                                        className="px-4 py-3 border-b last:border-0 align-middle leading-relaxed"
                                        style={{ borderColor: "var(--border-subtle)", color: "var(--ai-bubble-fg)" }}
                                        {...props}
                                      />
                                    ),
                                    tr: ({ node, ...props }) => (
                                      <tr className="hover:opacity-90 transition-opacity" {...props} />
                                    ),
                                    a: ({ href, children }) => {
                                      const handleClick = (e: React.MouseEvent) => {
                                        e.preventDefault();
                                        if (!href) return;
                                        // Windows absolute paths or file:// → open with native opener
                                        if (/^[A-Za-z]:[\\\/]/.test(href) || href.startsWith("file://")) {
                                          const localPath = href.startsWith("file://")
                                            ? decodeURIComponent(href.replace(/^file:\/\/\/?/, "").replace(/\//g, "\\"))
                                            : href;
                                          invoke("open_local_file", { path: localPath }).catch(err =>
                                            alert("فشل فتح الملف: " + err)
                                          );
                                        } else {
                                          // External HTTP(S) links → open in default browser
                                          invoke("open_local_file", { path: href }).catch(() => {
                                            window.open(href, "_blank", "noopener,noreferrer");
                                          });
                                        }
                                      };
                                      return (
                                        <a
                                          href={href}
                                          onClick={handleClick}
                                          className="underline cursor-pointer font-semibold"
                                          style={{ color: "var(--brand-accent)" }}
                                        >
                                          {children}
                                        </a>
                                      );
                                    },
                                  }}
                                >
                                  {content}
                               </ReactMarkdown>
                            </div>
                            {msg.aiUsage && (
                              <div
                                className="flex flex-wrap items-center gap-x-3 gap-y-1 rounded-lg border px-3 py-2 text-[11px]"
                                style={{
                                  background: "var(--bg-muted)",
                                  borderColor: "var(--border-subtle)",
                                  color: "var(--fg-2)",
                                }}
                                dir="ltr"
                              >
                                <span className="font-mono">{msg.aiUsage.model}</span>
                                <span className="font-mono">
                                  tokens: {String(msg.aiUsage.totalTokens)}
                                </span>
                                <span className="font-mono">
                                  in: {String(msg.aiUsage.promptTokens)}
                                </span>
                                <span className="font-mono">
                                  out: {String(msg.aiUsage.completionTokens)}
                                </span>
                                <span className="font-mono">
                                  source: {msg.aiUsage.usageSource || "usage"}
                                </span>
                              </div>
                            )}
                            {filePath && (
                                <button
                                    onClick={() => invoke("open_local_file", { path: filePath }).catch(err => alert("فشل فتح الملف: " + err))}
                                    className="self-start flex items-center gap-2 px-4 py-2 rounded-lg transition-colors text-sm font-semibold mt-2 shadow-sm border"
                                    style={{
                                      background: "var(--brand-accent-soft)",
                                      color: "var(--brand-accent-ink)",
                                      borderColor: "var(--ai-bubble-border)",
                                    }}
                                >
                                    <Download className="w-4 h-4" />
                                    فتح / معاينة الملف
                                </button>
                            )}
                          </div>
                      )}
                   </div>
                </div>
                );
             })}
             {isActiveChatLoading && (
                 <div className="flex justify-end">
                    <div className="max-w-[85%] rounded-2xl p-4 rounded-bl-sm flex items-center gap-3 border" style={{ background: 'var(--ai-bubble-bg)', borderColor: 'var(--ai-bubble-border)', color: 'var(--ai-bubble-fg)' }}>
                       <Loader2 className="w-5 h-5 animate-spin shrink-0" style={{ color: 'var(--brand-accent)' }} />
                       {toolProgress ? (
                           <span className="text-sm font-medium animate-pulse" style={{ color: 'var(--ai-bubble-fg)' }}>{toolProgress}</span>
                       ) : (
                           <span className="text-sm animate-pulse" style={{ color: 'var(--ai-bubble-fg)' }}>جاري التفكير...</span>
                       )}
                    </div>
                 </div>
             )}
             <div ref={chatEndRef} />
          </div>
        )}

        {/* Input + mention dropdown (wrapper must stay overflow-visible) */}
        <div className="w-full shrink-0 mt-auto mb-2 relative z-30">
          <AnimatePresence>
            {showMentionDrop && (
              <motion.div
                ref={mentionDropRef}
                initial={{ opacity: 0, y: 10, scale: 0.98 }}
                animate={{ opacity: 1, y: 0, scale: 1 }}
                exit={{ opacity: 0, y: 10, scale: 0.98 }}
                transition={{ duration: 0.15, ease: "easeOut" }}
                className="absolute bottom-[calc(100%+12px)] right-4 w-80 z-[100]"
              >
                <div className="rounded-2xl border border-border/60 bg-popover/95 backdrop-blur-xl text-popover-foreground shadow-2xl overflow-hidden">
                  <div className="px-4 py-3 border-b border-border/40 bg-muted/30 flex items-center justify-between gap-2 sticky top-0 z-10">
                    <span className="text-xs font-semibold text-muted-foreground flex items-center gap-1.5">
                      <Tag className="w-3.5 h-3.5 text-primary" />
                      اختر المنتج (أكمل للبحث)
                    </span>
                    {mentionLoading && <Loader2 className="w-3.5 h-3.5 animate-spin text-primary" />}
                  </div>
                  <div className="max-h-64 overflow-y-auto scrollbar-thin scrollbar-thumb-border scrollbar-track-transparent">
                  {productSuggestions.length === 0 && !mentionLoading ? (
                    <p className="px-4 py-8 text-sm text-center text-muted-foreground">لا توجد منتجات مطابقة</p>
                  ) : (
                    <ul className="py-1">
                      {productSuggestions.map((hit, idx) => (
                        <li key={`${hit.code}-${idx}`} className="relative">
                          {idx === mentionFocusIdx && (
                              <motion.div layoutId="mention-focus-bg" className="absolute inset-x-2 inset-y-0.5 bg-primary/10 rounded-lg -z-10 pointer-events-none" />
                          )}
                          <button
                            type="button"
                            onMouseDown={(e) => e.preventDefault()}
                            onClick={() => selectProductMention(hit)}
                            className={cn(
                              "w-full text-right px-4 py-2.5 flex flex-col gap-1 transition-colors relative z-10",
                              idx === mentionFocusIdx ? "text-primary" : "hover:bg-muted/40 text-foreground"
                            )}
                          >
                            <span className="text-sm font-semibold line-clamp-1 leading-tight">
                              {hit.name || "—"}
                            </span>
                            <span className="text-[11px] text-muted-foreground font-mono bg-muted/60 px-1.5 py-0.5 rounded shadow-sm w-fit">
                              {hit.code}
                            </span>
                          </button>
                        </li>
                      ))}
                    </ul>
                  )}
                  </div>
                </div>
              </motion.div>
            )}
          </AnimatePresence>
          <AnimatePresence initial={false} mode="wait">
            {!isInputCollapsed ? (
              <motion.div
                key="input-expanded"
                initial={{ opacity: 0, y: 20, height: 0 }}
                animate={{ opacity: 1, y: 0, height: "auto" }}
                exit={{ opacity: 0, y: 20, height: 0 }}
                transition={{ duration: 0.22, ease: [0.16, 1, 0.3, 1] }}
                className="overflow-visible"
              >
                {/* مقبض الإخفاء العلوي */}
                <div className="flex justify-center mb-1">
                  <button
                    type="button"
                    onClick={() => setIsInputCollapsed(true)}
                    title="إخفاء مربع الكتابة"
                    className="group flex items-center gap-1.5 px-3 py-1 rounded-full bg-muted/40 hover:bg-muted text-muted-foreground hover:text-foreground transition-all text-[11px] font-medium"
                  >
                    <ChevronDown className="w-3 h-3 group-hover:translate-y-0.5 transition-transform" />
                    إخفاء
                  </button>
                </div>

                <div className="bg-card border border-border rounded-xl shadow-sm flex items-end gap-2 p-2.5">
                  <textarea
                    ref={inputRef}
                    rows={1}
                    placeholder="اكتب رسالتك هنا… استخدم @ لاختيار منتج من قاعدة البيانات (Enter للإرسال، Shift+Enter سطر جديد)"
                    value={inputValue}
                    onChange={handleInputChange}
                    onSelect={handleInputSelect}
                    onClick={handleInputSelect}
                    onKeyDown={handleInputKeyDown}
                    className="flex-1 text-foreground bg-transparent text-base outline-none placeholder:text-muted-foreground resize-none min-h-[2.5rem] max-h-40 leading-relaxed px-2 py-1.5"
                    dir="rtl"
                  />
                  {isActiveChatLoading ? (
                    <button
                      type="button"
                      onClick={handleStopMessage}
                      title="إيقاف الرد"
                      className="shrink-0 w-9 h-9 flex items-center justify-center rounded-full bg-destructive text-destructive-foreground hover:opacity-90 transition-colors"
                    >
                      <Square className="w-4 h-4 fill-current" />
                    </button>
                  ) : (
                    <button
                      type="button"
                      onClick={handleSendMessage}
                      disabled={!inputValue.trim()}
                      className={`shrink-0 w-9 h-9 flex items-center justify-center rounded-full transition-colors ${
                        inputValue.trim()
                          ? "bg-primary text-primary-foreground hover:opacity-90"
                          : "bg-muted text-muted-foreground cursor-not-allowed"
                      }`}
                    >
                      <ArrowUp className="w-4 h-4" />
                    </button>
                  )}
                </div>
              </motion.div>
            ) : (
              <motion.div
                key="input-collapsed"
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: 20 }}
                transition={{ duration: 0.22, ease: [0.16, 1, 0.3, 1] }}
                className="flex justify-center"
              >
                <motion.button
                  type="button"
                  onClick={() => {
                    setIsInputCollapsed(false);
                    setTimeout(() => inputRef.current?.focus(), 250);
                  }}
                  whileHover={{ scale: 1.03 }}
                  whileTap={{ scale: 0.97 }}
                  title="إظهار مربع الكتابة"
                  className="group flex items-center gap-2 px-5 h-11 rounded-full bg-primary text-primary-foreground shadow-lg shadow-primary/25 hover:shadow-primary/40 transition-shadow text-sm font-bold"
                >
                  <ChevronUp className="w-4 h-4 group-hover:-translate-y-0.5 transition-transform" />
                  إظهار مربع الكتابة
                </motion.button>
              </motion.div>
            )}
          </AnimatePresence>
        </div>
        </div>
      </div>
    </div>
  );
}
