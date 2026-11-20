select x.x from f outer apply (select top 1 g.x from g where g.f_id = f.id) x where x.x <> 5
