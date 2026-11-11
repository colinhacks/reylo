select date_sub('day', coalesce(born, date '2000-01-01'), date '2020-01-01') as days, ntile(team_id) over () as bucket, ntile(2) over () as half, map([id], [name]) as names
from users where team_id is null or team_id > 0
