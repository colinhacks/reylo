select count(*) as c, (select count(*) from o where o.t_id = t.id) as n from t where t.id = 99
