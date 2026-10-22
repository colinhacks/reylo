select (select coalesce(json_arrayagg(json_object('n', p.n)), json_array()) from (select id from posts) p (n)) as ids
