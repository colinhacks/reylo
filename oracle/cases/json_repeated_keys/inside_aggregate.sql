select jsonb_agg(jsonb_build_object('k', id, 'k', name)) as v from t
