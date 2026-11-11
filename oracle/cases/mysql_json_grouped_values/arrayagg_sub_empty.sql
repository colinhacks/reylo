select (select json_object('a', json_arrayagg(x.v), 'm', max(x.v)) from nothing x group by x.id) as o
