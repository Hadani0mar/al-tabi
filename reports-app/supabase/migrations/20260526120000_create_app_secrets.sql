-- تخزين التوكنز الحساسة للتطبيق reports-app (delivery-app)
-- القراءة/الكتابة عبر RPC فقط — لا وصول مباشر من anon/authenticated

CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE IF NOT EXISTS public.app_access (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  name text NOT NULL DEFAULT 'default',
  token_hash text NOT NULL UNIQUE,
  is_active boolean NOT NULL DEFAULT true,
  created_at timestamptz NOT NULL DEFAULT now(),
  last_used_at timestamptz
);

CREATE TABLE IF NOT EXISTS public.app_secrets (
  secret_key text PRIMARY KEY,
  secret_value text NOT NULL,
  description text,
  is_sensitive boolean NOT NULL DEFAULT true,
  updated_at timestamptz NOT NULL DEFAULT now()
);

ALTER TABLE public.app_access ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.app_secrets ENABLE ROW LEVEL SECURITY;

CREATE OR REPLACE FUNCTION public.get_app_secrets(p_access_token text)
RETURNS jsonb
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

  IF NOT EXISTS (
    SELECT 1 FROM public.app_access
    WHERE token_hash = v_hash AND is_active = true
  ) THEN
    RAISE EXCEPTION 'invalid_access_token';
  END IF;

  UPDATE public.app_access
  SET last_used_at = now()
  WHERE token_hash = v_hash;

  RETURN COALESCE(
    (SELECT jsonb_object_agg(secret_key, secret_value) FROM public.app_secrets),
    '{}'::jsonb
  );
END;
$$;

CREATE OR REPLACE FUNCTION public.save_app_secrets(p_access_token text, p_secrets jsonb)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, extensions
AS $$
DECLARE
  v_hash text;
  v_key text;
  v_val text;
BEGIN
  IF p_access_token IS NULL OR length(trim(p_access_token)) < 16 THEN
    RAISE EXCEPTION 'access_token_too_short';
  END IF;

  IF p_secrets IS NULL OR p_secrets = '{}'::jsonb THEN
    RAISE EXCEPTION 'empty_secrets';
  END IF;

  v_hash := encode(digest(trim(p_access_token), 'sha256'), 'hex');

  IF NOT EXISTS (SELECT 1 FROM public.app_access) THEN
    INSERT INTO public.app_access (token_hash) VALUES (v_hash);
  ELSIF NOT EXISTS (
    SELECT 1 FROM public.app_access
    WHERE token_hash = v_hash AND is_active = true
  ) THEN
    RAISE EXCEPTION 'invalid_access_token';
  END IF;

  FOR v_key, v_val IN SELECT key, value FROM jsonb_each_text(p_secrets)
  LOOP
    INSERT INTO public.app_secrets (secret_key, secret_value, updated_at)
    VALUES (v_key, v_val, now())
    ON CONFLICT (secret_key) DO UPDATE
      SET secret_value = EXCLUDED.secret_value,
          updated_at = now();
  END LOOP;
END;
$$;

REVOKE ALL ON FUNCTION public.get_app_secrets(text) FROM PUBLIC;
REVOKE ALL ON FUNCTION public.save_app_secrets(text, jsonb) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION public.get_app_secrets(text) TO anon, authenticated;
GRANT EXECUTE ON FUNCTION public.save_app_secrets(text, jsonb) TO anon, authenticated;

COMMENT ON TABLE public.app_secrets IS
  'OpenRouter, OpenAI, Telegram keys for reports-app. Managed via save_app_secrets RPC.';
