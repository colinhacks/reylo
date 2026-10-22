select lag(n, 1, 0) over (order by id) as v from t
