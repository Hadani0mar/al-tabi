-- مهاجرة وتأمين مفاتيح التطبيق باستخدام ملحق Supabase Vault (pgsodium) بدلاً من الجدول العام
-- هذا يضمن تشفير المفاتيح بالكامل عند السكون (at rest) وعزلها بشكل كامل وآمن.

DO $$
DECLARE
  v_rec RECORD;
  v_secret_id uuid;
BEGIN
  -- 1. نقل المفاتيح الموجودة حالياً من الجدول العام إلى Supabase Vault
  IF EXISTS (SELECT 1 FROM pg_tables WHERE schemaname = 'public' AND tablename = 'app_secrets') THEN
    FOR v_rec IN SELECT secret_key, secret_value FROM public.app_secrets
    LOOP
      SELECT id INTO v_secret_id FROM vault.secrets WHERE name = 'reports_app:' || v_rec.secret_key;
      IF v_secret_id IS NOT NULL THEN
        PERFORM vault.update_secret(v_secret_id, new_secret := v_rec.secret_value);
      ELSE
        PERFORM vault.create_secret(
          new_secret := v_rec.secret_value, 
          new_name := 'reports_app:' || v_rec.secret_key, 
          new_description := 'Key for reports-app'
        );
      END IF;
    END LOOP;
    
    -- 2. حذف الجدول العام تماماً لضمان عدم وجود أي مفاتيح بنص واضح
    DROP TABLE IF EXISTS public.app_secrets;
  END IF;
END;
$$;

CREATE OR REPLACE FUNCTION public.get_app_secrets(p_access_token text)
RETURNS jsonb
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, extensions, vault
AS $$
DECLARE
  v_hash text;
  v_openrouter text;
  v_openai text;
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

  -- البحث أولاً بالبادئة reports_app: ثم الاسم المباشر كاحتياطي لتوافق واجهة سوبابيز
  SELECT decrypted_secret INTO v_openrouter
  FROM vault.decrypted_secrets
  WHERE name = 'reports_app:openrouter_api_key'
  LIMIT 1;

  IF v_openrouter IS NULL THEN
    SELECT decrypted_secret INTO v_openrouter
    FROM vault.decrypted_secrets
    WHERE name = 'openrouter_api_key'
    LIMIT 1;
  END IF;

  SELECT decrypted_secret INTO v_openai
  FROM vault.decrypted_secrets
  WHERE name = 'reports_app:openai_api_key'
  LIMIT 1;

  IF v_openai IS NULL THEN
    SELECT decrypted_secret INTO v_openai
    FROM vault.decrypted_secrets
    WHERE name = 'openai_api_key'
    LIMIT 1;
  END IF;

  RETURN jsonb_build_object(
    'openrouter_api_key', COALESCE(v_openrouter, ''),
    'openai_api_key', COALESCE(v_openai, '')
  );
END;
$$;

-- 4. تحديث دالة save_app_secrets لتخزن/تحدث المفاتيح داخل Vault
CREATE OR REPLACE FUNCTION public.save_app_secrets(p_access_token text, p_secrets jsonb)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, extensions, vault
AS $$
DECLARE
  v_hash text;
  v_key text;
  v_val text;
  v_secret_id uuid;
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
    SELECT id INTO v_secret_id FROM vault.secrets WHERE name = 'reports_app:' || v_key;
    
    IF v_secret_id IS NOT NULL THEN
      PERFORM vault.update_secret(v_secret_id, new_secret := v_val);
    ELSE
      PERFORM vault.create_secret(
        new_secret := v_val, 
        new_name := 'reports_app:' || v_key, 
        new_description := 'Key for reports-app'
      );
    END IF;
  END LOOP;
END;
$$;

-- 5. إعادة تعيين الصلاحيات الآمنة للدوال
REVOKE ALL ON FUNCTION public.get_app_secrets(text) FROM PUBLIC;
REVOKE ALL ON FUNCTION public.save_app_secrets(text, jsonb) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION public.get_app_secrets(text) TO anon, authenticated;
GRANT EXECUTE ON FUNCTION public.save_app_secrets(text, jsonb) TO anon, authenticated;
