select d.n, x.email || '!' as label from users x left join (select id, id + 1 as n from users) d on d.id = x.id + 1000
