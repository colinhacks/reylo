with o as (select n from other where id = 1) select c.id from o, c where o.n = c.b
