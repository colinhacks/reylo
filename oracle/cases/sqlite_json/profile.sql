select json_object(
  'user', json_object('id', u.id, 'name', u.name),
  'posts', json((select json_group_array(json_object('id', p.id, 'title', p.title)) from posts p where p.user_id = u.id))
) as profile
from users u where u.id = :id
