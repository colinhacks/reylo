select p.id, (select coalesce(json_arrayagg(json_object('id', c.id, 'body', c.body)), json_array()) from (select c.id, c.body from comment c where c.post_id = p.id) c) as comments from post p
