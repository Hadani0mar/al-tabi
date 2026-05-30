-- مشاركة المنتجات: بيانات النشاط + تحكم فوري (بدون كميات)

DROP FUNCTION IF EXISTS public.search_medicines(text);

CREATE OR REPLACE FUNCTION public.update_pharmacy_profile(
  p_sync_key text,
  p_name text,
  p_city text DEFAULT NULL,
  p_address text DEFAULT NULL,
  p_phone text DEFAULT NULL,
  p_show_prices boolean DEFAULT NULL,
  p_is_active boolean DEFAULT NULL
)
RETURNS jsonb
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
DECLARE
  v_id uuid;
BEGIN
  IF p_sync_key IS NULL OR length(trim(p_sync_key)) < 8 THEN
    RETURN jsonb_build_object('success', false, 'error', 'invalid sync_key');
  END IF;
  IF p_name IS NULL OR length(trim(p_name)) < 2 THEN
    RETURN jsonb_build_object('success', false, 'error', 'invalid name');
  END IF;

  UPDATE public.pharmacies
  SET
    name = trim(p_name),
    city = COALESCE(NULLIF(trim(p_city), ''), city),
    address = COALESCE(p_address, address),
    phone = COALESCE(p_phone, phone),
    show_prices = COALESCE(p_show_prices, show_prices),
    show_quantities = false,
    show_expiry = false,
    is_active = COALESCE(p_is_active, is_active)
  WHERE sync_key = trim(p_sync_key)
  RETURNING id INTO v_id;

  IF v_id IS NULL THEN
    RETURN jsonb_build_object('success', false, 'error', 'invalid sync_key');
  END IF;

  RETURN jsonb_build_object('success', true, 'pharmacy_id', v_id);
END;
$$;

CREATE OR REPLACE FUNCTION public.clear_pharmacy_products(p_sync_key text)
RETURNS jsonb
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
DECLARE
  v_id uuid;
  v_deleted integer;
BEGIN
  IF p_sync_key IS NULL OR length(trim(p_sync_key)) < 8 THEN
    RETURN jsonb_build_object('success', false, 'error', 'invalid sync_key');
  END IF;

  SELECT id INTO v_id
  FROM public.pharmacies
  WHERE sync_key = trim(p_sync_key);

  IF v_id IS NULL THEN
    RETURN jsonb_build_object('success', false, 'error', 'invalid sync_key');
  END IF;

  DELETE FROM public.pharmacy_products WHERE pharmacy_id = v_id;
  GET DIAGNOSTICS v_deleted = ROW_COUNT;

  UPDATE public.pharmacies
  SET last_sync_at = NULL, is_active = false, show_quantities = false
  WHERE id = v_id;

  RETURN jsonb_build_object('success', true, 'deleted', v_deleted);
END;
$$;

CREATE OR REPLACE FUNCTION public.upsert_pharmacy_products(p_sync_key text, p_products jsonb)
RETURNS jsonb
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
DECLARE
  v_pharmacy_id uuid;
  v_count integer := 0;
BEGIN
  IF p_sync_key IS NULL OR length(trim(p_sync_key)) < 8 THEN
    RETURN jsonb_build_object('success', false, 'error', 'invalid sync_key');
  END IF;

  SELECT id INTO v_pharmacy_id
  FROM public.pharmacies
  WHERE sync_key = trim(p_sync_key) AND is_active = TRUE;

  IF v_pharmacy_id IS NULL THEN
    RETURN jsonb_build_object('success', false, 'error', 'invalid sync_key or pharmacy inactive');
  END IF;

  DELETE FROM public.pharmacy_products WHERE pharmacy_id = v_pharmacy_id;

  INSERT INTO public.pharmacy_products (
    pharmacy_id, product_code, product_name, product_name_en,
    price, quantity, unit, nearest_expiry, is_available, synced_at
  )
  SELECT DISTINCT ON (
    v_pharmacy_id,
    COALESCE(NULLIF(trim(elem->>'product_code'), ''), trim(elem->>'product_name'))
  )
    v_pharmacy_id,
    NULLIF(trim(elem->>'product_code'), ''),
    trim(elem->>'product_name'),
    NULLIF(trim(elem->>'product_name_en'), ''),
    NULLIF(elem->>'price', '')::numeric,
    0,
    NULLIF(trim(elem->>'unit'), ''),
    NULL,
    COALESCE((elem->>'is_available')::boolean, true),
    NOW()
  FROM jsonb_array_elements(COALESCE(p_products, '[]'::jsonb)) AS elem
  WHERE length(trim(COALESCE(elem->>'product_name', ''))) > 0
  ORDER BY
    v_pharmacy_id,
    COALESCE(NULLIF(trim(elem->>'product_code'), ''), trim(elem->>'product_name')),
    trim(elem->>'product_name');

  GET DIAGNOSTICS v_count = ROW_COUNT;

  UPDATE public.pharmacies
  SET last_sync_at = NOW(), show_quantities = false, show_expiry = false
  WHERE id = v_pharmacy_id;

  RETURN jsonb_build_object('success', true, 'synced', v_count);
END;
$$;

CREATE OR REPLACE FUNCTION public.search_medicines(search_term text)
RETURNS TABLE(
  pharmacy_id uuid,
  pharmacy_name text,
  pharmacy_city text,
  pharmacy_address text,
  pharmacy_phone text,
  product_name text,
  product_name_en text,
  price numeric,
  show_prices boolean,
  is_available boolean,
  last_sync_at timestamp with time zone
)
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = public
AS $$
  SELECT
    p.id,
    p.name,
    p.city,
    p.address,
    p.phone,
    pp.product_name,
    pp.product_name_en,
    CASE WHEN p.show_prices THEN pp.price ELSE NULL END,
    p.show_prices,
    pp.is_available,
    p.last_sync_at
  FROM public.pharmacy_products pp
  JOIN public.pharmacies p ON p.id = pp.pharmacy_id
  WHERE
    p.is_active = TRUE
    AND pp.is_available = TRUE
    AND (
      pp.product_name ILIKE '%' || search_term || '%'
      OR pp.product_name_en ILIKE '%' || search_term || '%'
      OR COALESCE(pp.product_code, '') ILIKE '%' || search_term || '%'
    )
  ORDER BY similarity(pp.product_name, search_term) DESC, pp.product_name
  LIMIT 100;
$$;

REVOKE ALL ON FUNCTION public.update_pharmacy_profile(text, text, text, text, text, boolean, boolean) FROM PUBLIC;
REVOKE ALL ON FUNCTION public.clear_pharmacy_products(text) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION public.update_pharmacy_profile(text, text, text, text, text, boolean, boolean) TO anon, authenticated;
GRANT EXECUTE ON FUNCTION public.clear_pharmacy_products(text) TO anon, authenticated;
