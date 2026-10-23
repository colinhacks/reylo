select u.id,
  (select coalesce(string_agg(p.title, ', '), '') from posts p where p.user_id = u.id) as titles,
  (select coalesce(array_agg(p.id), '{}') from posts p where p.user_id = u.id) as ids,
  (select coalesce(bool_and(p.title <> ''), true) from posts p where p.user_id = u.id) as all_titled,
  (select coalesce(jsonb_object_agg(p.id, p.title), '{}') from posts p where p.user_id = u.id) as titles_by_id
from users u
