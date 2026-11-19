select d.n, (x.id > 5 || x.id < 0) as odd from users x left join (select id, id + 1 as n from users) d on d.id = x.id + 1000
