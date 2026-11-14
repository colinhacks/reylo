select coalesce(jsonb_agg(c), $$[]$$::jsonb) as doc from bodies_for(:p::int8) c
