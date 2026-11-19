select coalesce(year(dt), y) as c, (select max(year(d)) from days) as m from days
