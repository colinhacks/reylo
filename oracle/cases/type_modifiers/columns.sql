select name, code, flag, price, rounded, whole, at_time, at_zone, at, at_tz, flags, bits, tags, mail
from typed
where id = :id::int
