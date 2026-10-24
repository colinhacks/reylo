select p.user_id, json_objectagg(p.title, p.note) as by_title, json_arrayagg(p.note) as notes from posts p group by p.user_id
