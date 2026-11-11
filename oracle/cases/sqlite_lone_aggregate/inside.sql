select json_group_array(name) as names, count(a) as n, group_concat(name) as g from e
