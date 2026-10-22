select lag(n, 1, :p) over (order by id) as v from t
