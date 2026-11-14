select count(*) as c, (select json_group_array(e.name) filter (where o.z is not null) from opt o) as j from e where 0
