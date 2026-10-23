select coalesce(json_array(id), 'x') as a,
  case when id > 1 then json_object('a', 1) else json_object('b', 2) end as b,
  coalesce(doc, json_array()) as c,
  iif(id > 1, json_array(id), null) as d,
  coalesce(:doc::nullable(text), json_array()) as e,
  x from (select *, json_object('id', id, 'name', name) as x from users)
