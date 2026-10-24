select p.id, (select to_jsonb(c) from (select c.id, c.body from comment c where c.post_id = p.id limit 1) c) as latest from post p
