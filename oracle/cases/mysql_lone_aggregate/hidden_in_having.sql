select s, a from t where id = 99 having (select count(*) from o where o.id = max(t.id)) >= 0
