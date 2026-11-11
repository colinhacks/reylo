select iif(n > 0, n, json_extract(json_object('a', s), '$.a')) as v from t
