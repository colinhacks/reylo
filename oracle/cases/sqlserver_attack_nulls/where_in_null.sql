select g.x from f left join g on g.f_id = f.id where g.x not in (select x from g where 1 = 0)
