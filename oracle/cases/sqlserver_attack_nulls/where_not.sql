select g.x from f left join g on g.f_id = f.id where not (g.x is null)
