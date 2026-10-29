with c as (select id, total from orders) select a.id from c a left join (select id from c) b on b.id = a.id + 1
