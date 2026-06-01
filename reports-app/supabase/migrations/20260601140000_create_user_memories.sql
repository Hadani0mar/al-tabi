-- 1. تفريغ الذاكرة المشتركة القديمة بالكامل للبدء من الصفر بناءً على طلب المستخدم
TRUNCATE TABLE public.db_facts RESTART IDENTITY CASCADE;

-- 2. تمديد الـ Extensions اللازمة للـ Vector والتشفير
CREATE EXTENSION IF NOT EXISTS vector;
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- 3. إنشاء جدول الذاكرة الخاصة بكل مستخدم (معزولة ومحفوظة بالـ token_hash)
CREATE TABLE IF NOT EXISTS public.user_memories (
  id bigserial PRIMARY KEY,
  token_hash text NOT NULL,
  content text NOT NULL,
  category text NOT NULL DEFAULT 'preference',
  fingerprint text NOT NULL,
  embedding vector(1536),
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT unique_user_memory_fingerprint UNIQUE (token_hash, fingerprint)
);

-- 4. إنشاء جدول الدردشات ورسائل المستخدم والوكيل (الدردشات السحابية)
CREATE TABLE IF NOT EXISTS public.user_chats (
  id bigserial PRIMARY KEY,
  token_hash text NOT NULL,
  chat_id text NOT NULL,
  title text NOT NULL,
  messages jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT unique_user_chat_id UNIQUE (token_hash, chat_id)
);

-- 5. تفعيل RLS (Row Level Security) على الجداول الجديدة
ALTER TABLE public.user_memories ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.user_chats ENABLE ROW LEVEL SECURITY;

-- حظر الوصول المباشر من خلال REST لـ anon و authenticated
REVOKE ALL ON TABLE public.user_memories FROM PUBLIC, anon, authenticated;
REVOKE ALL ON TABLE public.user_chats FROM PUBLIC, anon, authenticated;

-- 6. دوال RPC للتعامل الآمن مع الذاكرة الخاصة للمستخدمين (عبر الرست وخادم MCP)

-- أ. حفظ ذاكرة خاصة لمستخدم مع ربطها وتجزئتها بالـ access_token
CREATE OR REPLACE FUNCTION public.upsert_user_memory(
  p_access_token text,
  p_content text,
  p_category text,
  p_fingerprint text,
  p_embedding vector(1536)
)
RETURNS bigint
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, extensions
AS $$
DECLARE
  v_hash text;
  v_id bigint;
BEGIN
  IF p_access_token IS NULL OR length(trim(p_access_token)) < 16 THEN
    RAISE EXCEPTION 'invalid_access_token';
  END IF;

  v_hash := encode(digest(trim(p_access_token), 'sha256'), 'hex');

  -- التحقق من نشاط التوكن
  IF NOT EXISTS (
    SELECT 1 FROM public.app_access
    WHERE token_hash = v_hash AND is_active = true
  ) THEN
    RAISE EXCEPTION 'invalid_access_token';
  END IF;

  IF p_content IS NULL OR length(trim(p_content)) < 4 THEN
    RAISE EXCEPTION 'content_too_short';
  END IF;

  INSERT INTO public.user_memories (token_hash, content, category, fingerprint, embedding, updated_at)
  VALUES (
    v_hash,
    left(trim(p_content), 2000),
    coalesce(nullif(trim(p_category), ''), 'preference'),
    trim(p_fingerprint),
    p_embedding,
    now()
  )
  ON CONFLICT (token_hash, fingerprint) DO UPDATE
    SET content = EXCLUDED.content,
        category = EXCLUDED.category,
        embedding = EXCLUDED.embedding,
        updated_at = now()
  RETURNING id INTO v_id;

  RETURN v_id;
END;
$$;

-- ب. البحث الاتجاهي RAG في الذاكرة الخاصة لمستخدم
CREATE OR REPLACE FUNCTION public.match_user_memories(
  p_access_token text,
  query_embedding vector(1536),
  match_threshold float DEFAULT 0.30,
  match_count int DEFAULT 5
)
RETURNS TABLE (
  id bigint,
  content text,
  category text,
  similarity float
)
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = public, extensions
AS $$
  SELECT
    m.id,
    m.content,
    m.category,
    1 - (m.embedding <=> query_embedding) AS similarity
  FROM public.user_memories m
  WHERE m.token_hash = encode(digest(trim(p_access_token), 'sha256'), 'hex')
    AND m.embedding IS NOT NULL
    AND 1 - (m.embedding <=> query_embedding) > match_threshold
  ORDER BY m.embedding <=> query_embedding
  LIMIT GREATEST(match_count, 1);
$$;

-- ج. مسح الذاكرة الخاصة بالكامل لمستخدم معين
CREATE OR REPLACE FUNCTION public.clear_user_memories(p_access_token text)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, extensions
AS $$
DECLARE
  v_hash text;
BEGIN
  IF p_access_token IS NULL OR length(trim(p_access_token)) < 16 THEN
    RAISE EXCEPTION 'invalid_access_token';
  END IF;

  v_hash := encode(digest(trim(p_access_token), 'sha256'), 'hex');

  DELETE FROM public.user_memories
  WHERE token_hash = v_hash;
END;
$$;


-- 7. دوال RPC للتعامل مع الدردشات السحابية للمستخدمين

-- أ. حفظ أو تحديث شات كامل
CREATE OR REPLACE FUNCTION public.upsert_user_chat(
  p_access_token text,
  p_chat_id text,
  p_title text,
  p_messages jsonb
)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, extensions
AS $$
DECLARE
  v_hash text;
BEGIN
  IF p_access_token IS NULL OR length(trim(p_access_token)) < 16 THEN
    RAISE EXCEPTION 'invalid_access_token';
  END IF;

  v_hash := encode(digest(trim(p_access_token), 'sha256'), 'hex');

  -- التحقق من نشاط التوكن
  IF NOT EXISTS (
    SELECT 1 FROM public.app_access
    WHERE token_hash = v_hash AND is_active = true
  ) THEN
    RAISE EXCEPTION 'invalid_access_token';
  END IF;

  INSERT INTO public.user_chats (token_hash, chat_id, title, messages, updated_at)
  VALUES (
    v_hash,
    trim(p_chat_id),
    trim(p_title),
    p_messages,
    now()
  )
  ON CONFLICT (token_hash, chat_id) DO UPDATE
    SET title = EXCLUDED.title,
        messages = EXCLUDED.messages,
        updated_at = now();
END;
$$;

-- ب. استرجاع جميع دردشات المستخدم مرتبة من الأحدث إلى الأقدم
CREATE OR REPLACE FUNCTION public.get_user_chats(p_access_token text)
RETURNS TABLE (
  chat_id text,
  title text,
  messages jsonb,
  updated_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, extensions
AS $$
DECLARE
  v_hash text;
BEGIN
  IF p_access_token IS NULL OR length(trim(p_access_token)) < 16 THEN
    RAISE EXCEPTION 'invalid_access_token';
  END IF;

  v_hash := encode(digest(trim(p_access_token), 'sha256'), 'hex');

  -- التحقق من نشاط التوكن
  IF NOT EXISTS (
    SELECT 1 FROM public.app_access
    WHERE token_hash = v_hash AND is_active = true
  ) THEN
    RAISE EXCEPTION 'invalid_access_token';
  END IF;

  RETURN QUERY
  SELECT c.chat_id, c.title, c.messages, c.updated_at
  FROM public.user_chats c
  WHERE c.token_hash = v_hash
  ORDER BY c.updated_at DESC;
END;
$$;

-- ج. حذف شات معين لمستخدم
CREATE OR REPLACE FUNCTION public.delete_user_chat(p_access_token text, p_chat_id text)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, extensions
AS $$
DECLARE
  v_hash text;
BEGIN
  IF p_access_token IS NULL OR length(trim(p_access_token)) < 16 THEN
    RAISE EXCEPTION 'invalid_access_token';
  END IF;

  v_hash := encode(digest(trim(p_access_token), 'sha256'), 'hex');

  DELETE FROM public.user_chats
  WHERE token_hash = v_hash AND chat_id = p_chat_id;
END;
$$;

-- 8. منح الصلاحيات لـ anon و authenticated لتشغيل الدوال الجديدة
GRANT EXECUTE ON FUNCTION public.upsert_user_memory(text, text, text, text, vector) TO anon, authenticated;
GRANT EXECUTE ON FUNCTION public.match_user_memories(text, vector, float, int) TO anon, authenticated;
GRANT EXECUTE ON FUNCTION public.clear_user_memories(text) TO anon, authenticated;

GRANT EXECUTE ON FUNCTION public.upsert_user_chat(text, text, text, jsonb) TO anon, authenticated;
GRANT EXECUTE ON FUNCTION public.get_user_chats(text) TO anon, authenticated;
GRANT EXECUTE ON FUNCTION public.delete_user_chat(text, text) TO anon, authenticated;

COMMENT ON TABLE public.user_memories IS 'Private user preferences and local memories, secured via SHA-256 token hashing.';
COMMENT ON TABLE public.user_chats IS 'Cloud-synced chat history for reports-app users, secured via SHA-256 token hashing.';
