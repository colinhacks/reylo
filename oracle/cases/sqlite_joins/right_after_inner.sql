-- An inner join's ON term removes rows, which a RIGHT JOIN after it null-extends again.
select u.email, p.title from users u join posts p on p.user_id = u.id and u.email is not null right join users r on r.id = p.id + 1000
