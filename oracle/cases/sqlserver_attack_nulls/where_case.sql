select g.x from f left join g on g.f_id = f.id where case when g.x is null then 1 else 1 end = 1
