select o.id, x.c from orders o left join lateral (select o.customer as c) x on false
