select f.id, x.mx from f cross apply (select max(g.x) as mx from g where g.f_id = f.id) x
