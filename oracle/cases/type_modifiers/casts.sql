select name::varchar(3) as short, price::numeric(11,1) as coarse, at::timestamp(0) as whole_second, flag::char(3) as padded, span::interval minute as by_minute
from typed
