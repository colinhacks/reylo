select replace(s, 'a', 'b') as r, replace(lower(s), 'a', 'b') as l, (s like 'x') as k, left(s, 1) as f from t
