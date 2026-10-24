select * from (select id, name from users) s where s.name = :n collate binary
