select json_object(
  'co', coalesce(json_array(id), 'x'),
  'ifn', ifnull(json_object('e', email), 'x'),
  'nul', nullif(json_array(id), 'x'),
  'iif', iif(id > 1, json_array(id), json_array(0)),
  'cs', case when id > 1 then json_object('x', id) end,
  'col', json_array(id) collate nocase,
  'pos', +json_array(id),
  'sub', json((select json_object('n', u2.name) from users u2 where u2.id = users.id)),
  'agg', json((select json_group_array(p.id) from posts p where p.user_id = users.id order by p.id)),
  'reach', coalesce(json((select json_group_array(p.title) from posts p where p.user_id = users.id)), '[]'),
  'mx', json((select max(json_array(p.id)) from posts p)),
  'jb', jsonb_array(id),
  'j', json(json_object('a', id))
) as o
from users
