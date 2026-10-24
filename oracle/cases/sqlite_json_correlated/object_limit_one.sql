select p.id, (select json_object('id', c.id, 'body', c.body) from (select c.id, c.body from comment c where c.post_id = p.id order by c.created_at desc limit 1) c) as latest from post p
