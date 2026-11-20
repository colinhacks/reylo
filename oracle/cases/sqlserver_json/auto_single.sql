select (select id, name from users for json auto) as people
