select u.name, s.title from users u left join (select id, title from posts) s on s.id = :p where u.id = :id
