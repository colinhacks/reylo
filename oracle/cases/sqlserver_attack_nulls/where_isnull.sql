select g.x + 1 as y from f left join g on g.f_id = f.id where isnull(g.x, 0) = 0
