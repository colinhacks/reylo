select sum(g.x) as s from f left join g on g.f_id = f.id where g.x = 1 group by f.id
