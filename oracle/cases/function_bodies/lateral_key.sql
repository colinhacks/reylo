select p.title, c.body from posts p, one_comment(p.id) c where p.id = :id::int8
