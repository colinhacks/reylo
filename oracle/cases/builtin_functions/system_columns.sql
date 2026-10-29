select e.xmin, e.ctid, e.tableoid, e.cmin, e.xmax, o.ctid as other, e.xmin::text as text from events e left join events o on o.id = e.id + 1
