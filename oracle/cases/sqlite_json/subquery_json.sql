select json_object(
  'sub', json((select json_object('n', u2.name) from users u2 where u2.id = users.id)),
  'agg', json((select json_group_array(p.id) from posts p where p.user_id = users.id order by p.id)),
  'reach', coalesce(json((select json_group_array(p.title) from posts p where p.user_id = users.id)), '[]'),
  'none', json((select json_object('n', u2.name) from users u2 where u2.id = -1))
) as o
from users
