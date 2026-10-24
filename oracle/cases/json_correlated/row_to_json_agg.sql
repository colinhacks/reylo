select p.id, (select coalesce(json_agg(row_to_json(c)), '[]') from (select c.id, c.body from comment c where c.post_id = p.id limit 2) c) as comments from post p
