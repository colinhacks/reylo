select count(*) as c, (select json_group_array(coalesce((select e.name where o.z is null), e.name)) from opt o) as j from e where 0
