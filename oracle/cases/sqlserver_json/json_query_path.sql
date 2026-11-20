select json_query(json_object('child': json_object('id': u.id)), '$.child') as c, json_query(json_object('id': u.id), '$') as whole from users u
