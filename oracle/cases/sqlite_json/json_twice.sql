select json(json((select json_object('a', id) from posts where 0))) as v, json_object('k', json(jsonb((select json_object('a', id) from posts where 0)))) as w from users
