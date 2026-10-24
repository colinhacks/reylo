select p.id, (select json_agg(json_build_object('id', c.id)) from (select c.id from comment c where c.post_id = p.id) c) as comments from post p
