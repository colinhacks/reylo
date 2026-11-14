select json_object('j', j) as v, json_group_array(jj) as g from (select j, jj from docs)
