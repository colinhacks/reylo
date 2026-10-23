select
  json_array((select u2.name from users u2 where u2.id = -1)) as a,
  json_object('a', json_array((select u2.name from users u2 where u2.id = -1))) as o,
  (select json_group_array((select u2.name from users u2 where u2.id = -1)) from users) as g
from users
