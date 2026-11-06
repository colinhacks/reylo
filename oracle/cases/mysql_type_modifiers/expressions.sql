select concat(name, '!') as shout, cast(name as char(3)) as short, coalesce(code, name) as either_one, upper(body) as loud, price * 2 as doubled, cast(price as decimal(6,1)) as coarse, at_ms + interval 1 day as tomorrow, timediff(span_ms, span) as gap
from sized
