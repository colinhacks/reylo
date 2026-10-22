select lead(n, 1, 'z') over (order by id) as v from t
