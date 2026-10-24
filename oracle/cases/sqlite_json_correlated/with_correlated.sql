select p.id, (with c as (select c.id, c.body from comment c where c.post_id = p.id limit 2) select json_group_array(json_object('id', c.id, 'body', c.body)) from c) as comments from post p
