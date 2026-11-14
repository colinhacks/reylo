select s, a + 1 as next, coalesce(s, 'x') as guarded, (select count(*) from o where o.id = max(t.id)) as n from t where id = 99
