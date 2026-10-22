select json_object(
  'from_subquery', s.x,
  'from_view', t.post,
  'sorted', (select json_object('id', u2.id) from users u2 order by u2.name limit 1),
  'scalar', (select json_object('n', u2.name) from users u2 where u2.id = s.id),
  'scalar_agg', (select json_group_array(p.id) from posts p where p.user_id = s.id),
  'scalar_coalesce', coalesce((select json_group_array(p.id) from posts p where p.user_id = s.id), json_array()),
  'compound', (select json_array(1) union all select json_array(2)),
  'lagged', lag(json_array(s.id)) over (order by s.id),
  'cast', cast(json_array(s.id) as text),
  'doc', json(s.doc),
  'extract', s.doc -> '$.a',
  'quoted', json_quote(s.name),
  'mixed', case when s.id > 1 then json_array(s.id) else 'x' end
) as o
from (select *, json_array(id) as x from users where doc is null or json_valid(doc)) s left join titled t on t.user_id = s.id
