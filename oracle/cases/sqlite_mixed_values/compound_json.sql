select n as v from t union all select json_extract(json_object('a', s), '$.a') from t
