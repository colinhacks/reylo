select p.id, (select coalesce(json_agg(json_build_object('id', c.id, 'body', c.body)), '[]') from (select c.id, c.body from comment c where c.post_id = p.id) c) as comments from post p
