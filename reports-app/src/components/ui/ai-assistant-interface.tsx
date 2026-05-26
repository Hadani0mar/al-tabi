"use client";

import type React from "react";
import { useState, useRef, useEffect, useCallback } from "react";
import {
  Search,
  Mic,
  ArrowUp,
  Plus,
  FileText,
  Code,
  BookOpen,
  PenTool,
  BrainCircuit,
  Sparkles,
  Loader2,
  Trash2,
  Download,
  Menu,
  MessageSquare,
  PlusCircle,
  Square,
  Tag,
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
  const [searchEnabled, setSearchEnabled] = useState(false);
  const [deepResearchEnabled, setDeepResearchEnabled] = useState(false);
  const [uploadedFiles, setUploadedFiles] = useState<string[]>([]);
  const [showUploadAnimation, setShowUploadAnimation] = useState(false);
  const [activeCommandCategory, setActiveCommandCategory] = useState<string | null>(null);
  
  const [chatHistory, setChatHistory] = useState<Message[]>([]);
  const [loadingChatIds, setLoadingChatIds] = useState<Set<string>>(new Set());
  const [toolProgress, setToolProgress] = useState<string | null>(null);

  const [chats, setChats] = useState<ChatSession[]>([]);
  const [activeChatId, setActiveChatId] = useState<string | null>(null);
  const [isSidebarOpen, setIsSidebarOpen] = useState(false);

  const inputRef = useRef<HTMLTextAreaElement>(null);
  const mentionDropRef = useRef<HTMLDivElement>(null);
  const chatEndRef = useRef<HTMLDivElement>(null);
  const pendingByChatRef = useRef<Record<string, string>>({});
  const loadingChatIdsRef = useRef<Set<string>>(new Set());
  const sendLockRef = useRef(false);
  const mentionDebounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const [productSuggestions, setProductSuggestions] = useState<ProductMention[]>([]);
  const [mentionLoading, setMentionLoading] = useState(false);
  const [showMentionDrop, setShowMentionDrop] = useState(false);
  const [mentionFocusIdx, setMentionFocusIdx] = useState(-1);
  const [mentionCtx, setMentionCtx] = useState<MentionContext | null>(null);

  const isActiveChatLoading =
    activeChatId !== null && loadingChatIds.has(activeChatId);

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
        const store = await load("chats.json");
        const savedChats = await store.get<ChatSession[]>("history");
        const lastId = await store.get<string>("lastActiveChatId");
        if (savedChats?.length) {
          setChats(savedChats);
          if (lastId) {
            const last = savedChats.find((c) => c.id === lastId);
            if (last) {
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

  const commandSuggestions = {
    learn: [
      "اعرض لي أفضل 10 عملاء لدينا",
      "ما هي المنتجات التي تقارب على الانتهاء؟",
      "اعرض تقرير مبيعات هذا الشهر",
      "من هو أفضل مورد للبنادول؟",
    ],
    code: [
      "اكتب لي استعلام SQL لمعرفة المبيعات",
      "كيف أبحث برقم الباركود؟",
      "أريد تقريراً للديون المستحقة",
      "كيف أقارن بين أسعار الموردين؟",
    ],
    write: [
      "اكتب لي رسالة تذكير للعملاء بالديون",
      "اكتب لي تقريراً مبهراً للإدارة",
      "صغ لي رسالة شكر لعميل مميز",
    ],
  };

  const handleUploadFile = () => {
    setShowUploadAnimation(true);
    setTimeout(() => {
      const newFile = `ملف_تصدير.pdf`;
      setUploadedFiles((prev) => [...prev, newFile]);
      setShowUploadAnimation(false);
    }, 1500);
  };

  const handleCommandSelect = (command: string) => {
    setInputValue(command);
    setActiveCommandCategory(null);
    if (inputRef.current) {
      inputRef.current.focus();
    }
  };

  const saveChatsToStore = async (newChats: ChatSession[]) => {
    try {
      const store = await load("chats.json");
      await store.set("history", newChats);
      await store.save();
    } catch (e) {
      console.error("Failed to save chats:", e);
    }
  };

  const handleNewChat = () => {
    setChatHistory([]);
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

  const handleStopMessage = async () => {
    if (!activeChatId) return;
    const requestId = pendingByChatRef.current[activeChatId];
    if (!requestId) return;
    try {
      await invoke("cancel_local_ai", { requestId });
    } catch (e) {
      console.error("cancel failed:", e);
    }
    clearChatLoading(activeChatId);
    delete pendingByChatRef.current[activeChatId];
    sendLockRef.current = false;
    setToolProgress(null);
    const stopMsg: Message = {
      role: "assistant",
      content: "⏹ تم إيقاف الرد. يمكنك إرسال رسالة جديدة.",
    };
    setChatHistory((hist) => {
      const updated = [...hist, stopMsg];
      setChats((prev) => {
        const newC = prev.map((c) =>
          c.id === activeChatId
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

    let currentChatId = activeChatId;
    let isNewChat = false;
    if (!currentChatId) {
      currentChatId = crypto.randomUUID();
      isNewChat = true;
    }

    if (loadingChatIdsRef.current.has(currentChatId)) return;

    sendLockRef.current = true;
    const userMessage = trimmed;
    setInputValue("");
    setShowMentionDrop(false);
    setMentionCtx(null);
    const newMsg: Message = { role: "user", content: userMessage };
    const newHistory = [...chatHistory, newMsg];
    setChatHistory(newHistory);
    setToolProgress(null);

    if (isNewChat) {
      setActiveChatId(currentChatId);
      saveLastActiveChatId(currentChatId);
    }

    const prevRequestId = pendingByChatRef.current[currentChatId];
    if (prevRequestId) {
      invoke("cancel_local_ai", { requestId: prevRequestId }).catch(console.error);
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
          model: "openai/gpt-4o-mini",
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

        if (pendingByChatRef.current[currentChatId] !== requestId) return;

        const assistMsg: Message = { role: "assistant", content: response };
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
        if (activeChatId === currentChatId) {
          setChatHistory(finalHistory);
        }
    } catch (e) {
        if (pendingByChatRef.current[currentChatId] !== requestId) return;
        console.error(e);
        const errText = String(e);
        const errMsg: Message = {
          role: "assistant",
          content: errText.includes("إيقاف")
            ? "⏹ تم إيقاف الرد. يمكنك إرسال رسالة جديدة."
            : `❌ عذراً، حدث خطأ: ${e}`,
        };
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
        if (activeChatId === currentChatId) {
          setChatHistory(finalHistory);
        }
    } finally {
        if (pendingByChatRef.current[currentChatId] === requestId) {
          delete pendingByChatRef.current[currentChatId];
        }
        clearChatLoading(currentChatId);
        sendLockRef.current = false;
        if (activeChatId === currentChatId) {
          setToolProgress(null);
        }
    }
  };

  const selectChat = (id: string) => {
    const chat = chats.find((c) => c.id === id);
    if (chat) {
      setChatHistory(chat.messages);
      setActiveChatId(chat.id);
      saveLastActiveChatId(chat.id);
      setIsSidebarOpen(false);
    }
  };

  const deleteChat = (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    const newChats = chats.filter(c => c.id !== id);
    setChats(newChats);
    saveChatsToStore(newChats);
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
        <div className="w-full max-w-4xl mx-auto flex items-center justify-between mb-4">
          <button onClick={() => setIsSidebarOpen(true)} className="p-2 hover:bg-muted rounded-md border shadow-sm bg-card transition-colors flex items-center gap-2">
            <Menu className="w-5 h-5" />
            <span className="text-sm font-medium">سجل المحادثات</span>
          </button>
          {activeChatId && (
            <button onClick={handleNewChat} className="p-2 hover:bg-muted rounded-md border shadow-sm bg-card transition-colors flex items-center gap-2">
              <Plus className="w-5 h-5" />
              <span className="text-sm font-medium hidden sm:inline">محادثة جديدة</span>
            </button>
          )}
        </div>

        <div className="w-full max-w-4xl mx-auto flex flex-col h-[calc(100vh-12rem)] min-h-0 overflow-visible">
        
        {/* Header / Logo */}
        {chatHistory.length === 0 && (
          <div className="flex flex-col items-center justify-center flex-1 animate-in fade-in zoom-in duration-500">
            <div className="mb-6 w-20 h-20 relative">
               <img src="/ai.svg" alt="AI" className="w-full h-full text-primary" />
            </div>
            <div className="mb-8 text-center">
              <motion.div
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.3 }}
                className="flex flex-col items-center"
              >
                <h1 className="text-3xl font-bold bg-clip-text text-transparent bg-gradient-to-l from-indigo-600 to-violet-500 mb-2">
                  كيف يمكنني مساعدتك اليوم؟
                </h1>
                <p className="text-muted-foreground max-w-md text-sm">
                  يمكنني الإجابة على أسئلتك وإعداد تقارير مفصلة من قاعدة بيانات النظام.
                </p>
              </motion.div>
            </div>

            {/* Command categories */}
            <div className="w-full max-w-2xl grid grid-cols-3 gap-3 mb-4">
              <CommandButton
                icon={<BookOpen className="w-5 h-5" />}
                label="استعلام"
                isActive={activeCommandCategory === "learn"}
                onClick={() => setActiveCommandCategory(activeCommandCategory === "learn" ? null : "learn")}
              />
              <CommandButton
                icon={<Code className="w-5 h-5" />}
                label="بحث متقدم"
                isActive={activeCommandCategory === "code"}
                onClick={() => setActiveCommandCategory(activeCommandCategory === "code" ? null : "code")}
              />
              <CommandButton
                icon={<PenTool className="w-5 h-5" />}
                label="كتابة"
                isActive={activeCommandCategory === "write"}
                onClick={() => setActiveCommandCategory(activeCommandCategory === "write" ? null : "write")}
              />
            </div>

            {/* Command suggestions */}
            <AnimatePresence>
              {activeCommandCategory && (
                <motion.div
                  initial={{ opacity: 0, height: 0 }}
                  animate={{ opacity: 1, height: "auto" }}
                  exit={{ opacity: 0, height: 0 }}
                  className="w-full max-w-2xl mb-6 overflow-hidden"
                >
                  <div className="bg-card rounded-xl border border-border shadow-sm overflow-hidden">
                    <div className="p-3 border-b border-border bg-muted/20">
                      <h3 className="text-sm font-semibold text-foreground">
                        {activeCommandCategory === "learn"
                          ? "اقتراحات الاستعلام والتقارير"
                          : activeCommandCategory === "code"
                          ? "اقتراحات البحث وقواعد البيانات"
                          : "اقتراحات الصياغة والمراسلات"}
                      </h3>
                    </div>
                    <ul className="divide-y divide-border">
                      {commandSuggestions[activeCommandCategory as keyof typeof commandSuggestions].map((suggestion, index) => (
                        <motion.li
                          key={index}
                          initial={{ opacity: 0 }}
                          animate={{ opacity: 1 }}
                          transition={{ delay: index * 0.03 }}
                          onClick={() => handleCommandSelect(suggestion)}
                          className="p-3 hover:bg-muted/50 cursor-pointer transition-colors duration-75"
                        >
                          <div className="flex items-center gap-3">
                            <Sparkles className="w-4 h-4 text-indigo-500" />
                            <span className="text-sm text-foreground">
                              {suggestion}
                            </span>
                          </div>
                        </motion.li>
                      ))}
                    </ul>
                  </div>
                </motion.div>
              )}
            </AnimatePresence>
          </div>
        )}

        {/* Chat History */}
         {chatHistory.length > 0 && (
          <div className="flex-1 overflow-y-auto w-full max-w-4xl px-4 py-6 space-y-6 scrollbar-hide">
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
                   <div className={`max-w-[85%] rounded-2xl p-4 shadow-sm ${msg.role === 'user' ? 'bg-primary text-primary-foreground rounded-br-sm' : 'bg-card border border-border text-foreground rounded-bl-sm'}`}>
                      {msg.role === 'user' ? (
                          <div className="text-sm whitespace-pre-wrap">{content}</div>
                      ) : (
                          <div className="flex flex-col gap-3">
                            <div className="prose prose-sm dark:prose-invert max-w-none prose-p:leading-relaxed prose-pre:bg-muted prose-pre:border prose-pre:border-border prose-pre:text-foreground">
                               <ReactMarkdown
                                  remarkPlugins={[remarkGfm]}
                                  components={{
                                    table: ({ node, ...props }) => (
                                      <div className="w-full overflow-x-auto my-5 rounded-xl border border-border/60 shadow-sm bg-card/50">
                                        <table className="w-full text-sm text-right" {...props} />
                                      </div>
                                    ),
                                    thead: ({ node, ...props }) => <thead className="bg-muted/60 text-muted-foreground text-xs uppercase tracking-wider" {...props} />,
                                    th: ({ node, ...props }) => <th className="px-4 py-3 font-semibold border-b border-border/60 whitespace-nowrap" {...props} />,
                                    td: ({ node, ...props }) => <td className="px-4 py-3 border-b border-border/40 last:border-0 align-middle" {...props} />,
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
                                          className="text-indigo-500 underline cursor-pointer"
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
                            {filePath && (
                                <button
                                    onClick={() => invoke("open_local_file", { path: filePath }).catch(err => alert("فشل فتح الملف: " + err))}
                                    className="self-start flex items-center gap-2 px-4 py-2 bg-indigo-500/10 text-indigo-500 border border-indigo-500/20 rounded-lg hover:bg-indigo-500/20 transition-colors text-sm font-semibold mt-2 shadow-sm"
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
                    <div className="max-w-[85%] rounded-2xl p-4 bg-card border border-border text-foreground rounded-bl-sm flex items-center gap-3">
                       <Loader2 className="w-5 h-5 animate-spin text-primary" />
                       {toolProgress ? (
                           <span className="text-sm font-medium text-primary animate-pulse">{toolProgress}</span>
                       ) : (
                           <span className="text-sm text-muted-foreground animate-pulse">جاري التفكير...</span>
                       )}
                    </div>
                 </div>
             )}
             <div ref={chatEndRef} />
          </div>
        )}

        {/* Input + mention dropdown (wrapper must stay overflow-visible) */}
        <div className="w-full max-w-4xl shrink-0 mt-auto mb-2 relative z-30">
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
                      <Tag className="w-3.5 h-3.5 text-indigo-500" />
                      اختر المنتج (أكمل للبحث)
                    </span>
                    {mentionLoading && <Loader2 className="w-3.5 h-3.5 animate-spin text-indigo-500" />}
                  </div>
                  <div className="max-h-64 overflow-y-auto scrollbar-thin scrollbar-thumb-border scrollbar-track-transparent">
                  {productSuggestions.length === 0 && !mentionLoading ? (
                    <p className="px-4 py-8 text-sm text-center text-muted-foreground">لا توجد منتجات مطابقة</p>
                  ) : (
                    <ul className="py-1">
                      {productSuggestions.map((hit, idx) => (
                        <li key={`${hit.code}-${idx}`} className="relative">
                          {idx === mentionFocusIdx && (
                              <motion.div layoutId="mention-focus-bg" className="absolute inset-x-2 inset-y-0.5 bg-indigo-500/10 rounded-lg -z-10 pointer-events-none" />
                          )}
                          <button
                            type="button"
                            onMouseDown={(e) => e.preventDefault()}
                            onClick={() => selectProductMention(hit)}
                            className={cn(
                              "w-full text-right px-4 py-2.5 flex flex-col gap-1 transition-colors relative z-10",
                              idx === mentionFocusIdx ? "text-indigo-600 dark:text-indigo-400" : "hover:bg-muted/40 text-foreground"
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
          <div className="bg-card border border-border rounded-xl shadow-sm">
          <div className="p-4">
            <textarea
              ref={inputRef}
              rows={2}
              placeholder="اكتب رسالتك هنا… استخدم @ لاختيار منتج من قاعدة البيانات (Enter للإرسال، Shift+Enter سطر جديد)"
              value={inputValue}
              onChange={handleInputChange}
              onSelect={handleInputSelect}
              onClick={handleInputSelect}
              onKeyDown={handleInputKeyDown}
              className="w-full text-foreground bg-transparent text-base outline-none placeholder:text-muted-foreground resize-none min-h-[3rem] leading-relaxed"
              dir="rtl"
            />
          </div>

          {/* Uploaded files */}
          {uploadedFiles.length > 0 && (
            <div className="px-4 pb-3">
              <div className="flex flex-wrap gap-2">
                {uploadedFiles.map((file, index) => (
                  <div
                    key={index}
                    className="flex items-center gap-2 bg-muted/50 py-1 px-2 rounded-md border border-border"
                  >
                    <FileText className="w-3 h-3 text-indigo-500" />
                    <span className="text-xs text-foreground">{file}</span>
                    <button
                      onClick={() => setUploadedFiles((prev) => prev.filter((_, i) => i !== index))}
                      className="text-muted-foreground hover:text-destructive"
                    >
                      <Trash2 className="w-3 h-3" />
                    </button>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Tools & Send */}
          <div className="px-4 py-3 flex items-center justify-between border-t border-border/50">
            <div className="flex items-center gap-2">
              <button
                onClick={() => setSearchEnabled(!searchEnabled)}
                className={`flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs font-medium transition-colors ${
                  searchEnabled ? "bg-indigo-50 text-indigo-600 dark:bg-indigo-500/20 dark:text-indigo-400" : "bg-muted/50 text-muted-foreground hover:bg-muted"
                }`}
              >
                <Search className="w-3.5 h-3.5" />
                <span>بحث الويب</span>
              </button>
              <button
                onClick={() => setDeepResearchEnabled(!deepResearchEnabled)}
                className={`flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs font-medium transition-colors ${
                  deepResearchEnabled ? "bg-indigo-50 text-indigo-600 dark:bg-indigo-500/20 dark:text-indigo-400" : "bg-muted/50 text-muted-foreground hover:bg-muted"
                }`}
              >
                <BrainCircuit className="w-3.5 h-3.5" />
                <span>بحث عميق</span>
              </button>
              
               <button
                  onClick={handleUploadFile}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs font-medium bg-muted/50 text-muted-foreground hover:bg-muted transition-colors"
                >
                  {showUploadAnimation ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Plus className="w-3.5 h-3.5" />}
                  <span>رفع مستند</span>
               </button>
            </div>
            <div className="flex items-center gap-2">
              <button className="p-2 text-muted-foreground hover:text-foreground transition-colors rounded-full hover:bg-muted/50">
                <Mic className="w-5 h-5" />
              </button>
              {isActiveChatLoading ? (
                <button
                  type="button"
                  onClick={handleStopMessage}
                  title="إيقاف الرد"
                  className="w-10 h-10 flex items-center justify-center rounded-full bg-destructive text-destructive-foreground hover:opacity-90 transition-colors"
                >
                  <Square className="w-4 h-4 fill-current" />
                </button>
              ) : (
                <button
                  type="button"
                  onClick={handleSendMessage}
                  disabled={!inputValue.trim()}
                  className={`w-10 h-10 flex items-center justify-center rounded-full transition-colors ${
                    inputValue.trim()
                      ? "bg-primary text-primary-foreground hover:opacity-90"
                      : "bg-muted text-muted-foreground cursor-not-allowed"
                  }`}
                >
                  <ArrowUp className="w-4 h-4" />
                </button>
              )}
            </div>
          </div>
          </div>
        </div>
        </div>
      </div>
    </div>
  );
}

interface CommandButtonProps {
  icon: React.ReactNode;
  label: string;
  isActive: boolean;
  onClick: () => void;
}

function CommandButton({ icon, label, isActive, onClick }: CommandButtonProps) {
  return (
    <motion.button
      onClick={onClick}
      className={`flex flex-col items-center justify-center gap-2 p-4 rounded-xl border transition-all ${
        isActive
          ? "bg-primary/5 border-primary/30 shadow-sm"
          : "bg-card border-border hover:border-primary/50"
      }`}
    >
      <div className={`${isActive ? "text-primary" : "text-muted-foreground"}`}>
        {icon}
      </div>
      <span
        className={`text-sm font-medium ${
          isActive ? "text-primary" : "text-foreground"
        }`}
      >
        {label}
      </span>
    </motion.button>
  );
}
