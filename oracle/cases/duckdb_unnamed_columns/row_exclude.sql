select * exclude (id, name, email) from (select 1 as id, 'a' as name, 'b' as email, 2 as kept)
