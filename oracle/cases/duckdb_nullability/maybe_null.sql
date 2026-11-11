select id / 2 as half, id // 2 as whole, id % 2 as odd, try_cast(name as integer) as numeric_name, year(born) as birth_year,
  case when id > 1 then name end as late_name, upper(email) as shouting, (select max(id) from teams) as top_team
from users
