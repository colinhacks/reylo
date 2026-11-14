select ifnull(dt, 0) as v, case when ok then d else 0 end as w, iif(id > 0, d, 0.5) as x, coalesce(dt, 1.5) as z from dts
