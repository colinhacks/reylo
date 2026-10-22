select case when n > 0 then n else json_extract(json_object('a', s), '$.a') end as v from t
