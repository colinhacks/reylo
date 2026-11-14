select json_object('due', ifnull(dt, 0), 'on', case when ok then d else 0 end) as v from dts
