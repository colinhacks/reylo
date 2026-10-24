select p.id, (select json_group_array(json_object('id', c.id, 'body', c.body)) from (select c.id, c.body from comment c order by c.created_at desc limit 2) c) as comments from post p
