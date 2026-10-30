select a.n, v.column1 from (select count(*) as n from users) a, (values (1), (2)) v
