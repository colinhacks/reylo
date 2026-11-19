select json_object('payload', d.payload) as o from (select payload from events) d
