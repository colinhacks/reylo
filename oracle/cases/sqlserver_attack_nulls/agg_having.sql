select max(g.x) as m from f left join g on g.f_id = f.id group by f.id having count(*) > 0
