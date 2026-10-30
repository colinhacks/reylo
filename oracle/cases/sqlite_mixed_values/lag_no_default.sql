select lag(n) over (order by id) as v from t
