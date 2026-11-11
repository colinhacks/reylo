select a.n, b.m from (select count(*) as n from users) a join (select count(*) as m from posts) b on 1 = 2
