select lag(s, 1, x'00') over (order by id) as v from t
