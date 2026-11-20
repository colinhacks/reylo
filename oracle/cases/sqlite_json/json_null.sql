-- A JSON value is JSON's null when the document it holds is `null`, however NOT NULL it is: inside
-- built JSON it may be null.
select json_object('k', json('null'), 'p', 'null' -> '$', 'id', id) as o, json_array(json('null')) as t from users
