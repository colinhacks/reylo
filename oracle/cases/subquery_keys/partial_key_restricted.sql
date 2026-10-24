select * from (select id, name from users where deleted_at is null) s where s.name = :n
