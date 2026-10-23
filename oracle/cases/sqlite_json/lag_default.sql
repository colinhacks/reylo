select
  lag(json_array(id), 1, '{"a":1}') over (order by id) as other,
  lead(json_array(id), 1, json_array(0)) over (order by id) as same,
  json_object(
    'lagged', json(lag(json_array(id)) over (order by id)),
    'other', json(lag(json_array(id), 1, '{"a":1}') over (order by id)),
    'same', json(lead(json_array(id), 1, json_array(0)) over (order by id))
  ) as o
from users
