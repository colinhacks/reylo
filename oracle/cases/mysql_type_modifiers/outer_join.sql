select s.name, t.code, t.body, t.huge, t.at_us
from sized s
left join sized t on t.id = s.id + 1
