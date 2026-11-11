select id + 1 as next, id * 2 as twice, -id as negative, name || '!' as loud, coalesce(email, name) as contact, lower(name) as lowered,
  email is null as no_email, id in (1, 2) as early, cast(id as varchar) as id_text, case when id > 1 then name else 'first' end as label
from users
