select p.id, (select coalesce(json_agg(json_build_object('id', c.id, 'author', a.first_name)), '[]') from comment c join person a on a.id = c.author_id and c.post_id = p.id) as comments from post p
