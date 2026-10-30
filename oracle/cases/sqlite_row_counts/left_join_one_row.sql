select a.n, u.name from users u left join (select count(*) as n from users) a on 1 where u.id = :id
