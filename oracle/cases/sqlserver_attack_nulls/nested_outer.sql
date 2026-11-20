select g.x, h.x as hx from f left join (g inner join g as h on h.id = g.id) on g.f_id = f.id
