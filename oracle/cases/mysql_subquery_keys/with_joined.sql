with s as (select id, title from posts) select s.title, u.name from s join users u on u.id = :u where s.id = :id
