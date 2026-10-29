with c as (select id, total from orders) select a.id, a.total from c a left join c b on b.id = a.id + 1
