select to_jsonb(t) as j, row_to_json(t) as r, count(*) over () as n from t
