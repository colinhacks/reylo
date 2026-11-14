select p.id, c.id as comment from posts p, one_comment(p.id) c where p.id = c.id
