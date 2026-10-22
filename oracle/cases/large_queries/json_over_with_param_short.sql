with recent as (select user_id, placed_at from orders where org_id = :org_id)
select u.id, (select json_agg(json_build_object('at', r.placed_at)) from recent r where r.user_id = u.id) as recent
from users u
