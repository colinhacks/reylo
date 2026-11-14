select count(*) as n from comments c group by (select pick(c.id))
