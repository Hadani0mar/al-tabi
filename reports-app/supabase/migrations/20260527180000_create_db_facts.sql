-- حقائق مشتركة عن قاعدة البيانات Marketing2026 (ذاكرة الوكيل — جميع المستخدمين)
-- القراءة/الكتابة عبر RPC فقط

CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE IF NOT EXISTS public.db_facts (
  id bigserial PRIMARY KEY,
  content text NOT NULL,
  category text NOT NULL DEFAULT 'db_rule',
  fingerprint text NOT NULL UNIQUE,
  embedding vector(1536),
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

ALTER TABLE public.db_facts ENABLE ROW LEVEL SECURITY;

CREATE OR REPLACE FUNCTION public.match_db_facts(
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
SET search_path = public
AS $$
  SELECT
    f.id,
    f.content,
    f.category,
    1 - (f.embedding <=> query_embedding) AS similarity
  FROM public.db_facts f
  WHERE f.embedding IS NOT NULL
    AND 1 - (f.embedding <=> query_embedding) > match_threshold
  ORDER BY f.embedding <=> query_embedding
  LIMIT GREATEST(match_count, 1);
$$;

CREATE OR REPLACE FUNCTION public.upsert_db_fact(
  p_content text,
  p_category text,
  p_fingerprint text,
  p_embedding vector(1536)
)
RETURNS bigint
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
DECLARE
  v_id bigint;
BEGIN
  IF p_content IS NULL OR length(trim(p_content)) < 8 THEN
    RAISE EXCEPTION 'content_too_short';
  END IF;

  IF p_fingerprint IS NULL OR length(trim(p_fingerprint)) < 8 THEN
    RAISE EXCEPTION 'invalid_fingerprint';
  END IF;

  IF p_embedding IS NULL THEN
    RAISE EXCEPTION 'missing_embedding';
  END IF;

  INSERT INTO public.db_facts (content, category, fingerprint, embedding, updated_at)
  VALUES (
    left(trim(p_content), 2000),
    coalesce(nullif(trim(p_category), ''), 'db_rule'),
    trim(p_fingerprint),
    p_embedding,
    now()
  )
  ON CONFLICT (fingerprint) DO UPDATE
    SET content = EXCLUDED.content,
        category = EXCLUDED.category,
        embedding = EXCLUDED.embedding,
        updated_at = now()
  RETURNING id INTO v_id;

  RETURN v_id;
END;
$$;

REVOKE ALL ON TABLE public.db_facts FROM PUBLIC;
REVOKE ALL ON FUNCTION public.match_db_facts(vector, float, int) FROM PUBLIC;
REVOKE ALL ON FUNCTION public.upsert_db_fact(text, text, text, vector) FROM PUBLIC;

GRANT EXECUTE ON FUNCTION public.match_db_facts(vector, float, int) TO anon, authenticated;
GRANT EXECUTE ON FUNCTION public.upsert_db_fact(text, text, text, vector) TO anon, authenticated;

COMMENT ON TABLE public.db_facts IS
  'Shared agent memory: verified database schema/business facts for all reports-app users.';
