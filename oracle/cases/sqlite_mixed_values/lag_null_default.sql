select lag(n, 1, null) over (order by id) as v from t
