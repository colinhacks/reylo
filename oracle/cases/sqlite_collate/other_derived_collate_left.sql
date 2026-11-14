select c.id from (select b collate nocase as n from other where id = 1) o, c where o.n = c.b
