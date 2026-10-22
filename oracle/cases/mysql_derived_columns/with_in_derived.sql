with c as (select (select count(*) from posts) as n) select d.n from (select n from c) d
