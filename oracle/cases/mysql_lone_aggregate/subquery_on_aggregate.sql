select max(t.id) as m, (select count(*) from o where o.t_id = max(t.id)) as n from t where t.a > :x
