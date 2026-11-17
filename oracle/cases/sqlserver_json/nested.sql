select u.id, (select p.id, p.title, p.body, p.score, p.at, p.tag from posts p where p.user_id = u.id order by p.id for json path) as posts from users u
