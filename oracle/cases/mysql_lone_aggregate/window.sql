select t.id, t.s, count(*) over () as n from t where t.a > :x
