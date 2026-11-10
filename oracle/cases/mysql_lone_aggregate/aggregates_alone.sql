select count(*) as c, coalesce(max(a), 0) as m, coalesce((select max(o.id) from o), 0) as top, 7 as k, count(*) + 1 as more from t where id = 99
