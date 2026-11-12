select (select json_object('ids', json_group_array(p.id))) as doc, rank() over () as r from posts p
