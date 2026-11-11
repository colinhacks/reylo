select coalesce(s, json_extract(json_object('a', s), '$.a')) as v from t
