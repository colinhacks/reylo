select upper(name) as up, coalesce(name, name) as same, price * 2 as doubled, tags[1] as first_tag, max(name) over () as top
from typed
