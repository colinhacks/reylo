select a.title, b.name from (select id, title, author_id from posts) a join (select id, name from users) b on b.id = a.author_id where a.id = :id
