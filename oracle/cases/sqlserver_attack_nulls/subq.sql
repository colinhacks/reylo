select (select max(x) from g where g.f_id = -5) as a, (select x from g where g.id = -5) as b from f
