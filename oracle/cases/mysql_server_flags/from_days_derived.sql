select s.id, s.d from (select id, from_days(n) as d from t) s
