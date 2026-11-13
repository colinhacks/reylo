select json_group_array((select e.name)) as names, count(*) as c from e
