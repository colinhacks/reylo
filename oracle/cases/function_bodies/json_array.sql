select p.id, comments_json(p.id) as comments from posts p
