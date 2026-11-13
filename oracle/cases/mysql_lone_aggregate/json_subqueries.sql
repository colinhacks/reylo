select max(a) as c, (select json_object('n', count(*)) from o where o.t_id = t.id) as j, (select coalesce(json_arrayagg(o.id), json_array()) from o where o.t_id = t.id) as ja from t where t.id = 99
