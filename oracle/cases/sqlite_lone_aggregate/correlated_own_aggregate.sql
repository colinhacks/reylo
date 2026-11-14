select count(*) as c, (select json_group_array(o.z || e.name) from opt o) as j, (select json_group_array(e.name) from opt o) as k from e where 0
