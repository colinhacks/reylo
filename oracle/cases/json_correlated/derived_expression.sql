select p.id, (select coalesce(json_agg(json_build_object('n', c.n, 'title', p.title)), '[]') from (select c.id + p.id as n from comment c where c.post_id = p.id) c) as comments from post p
