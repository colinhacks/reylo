select s.id, s.n, s.e from (select id, name as n, email as e from users) s
