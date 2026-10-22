select d.n, count(*) as c from (select (select count(*) from posts) as n from users) d group by d.n with rollup
