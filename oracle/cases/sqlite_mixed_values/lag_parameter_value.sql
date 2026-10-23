select lag(:p, 1, n) over (order by id) as v from t
