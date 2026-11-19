select json_arrayagg(json_object('id', id, 'payload', payload)) as o from events
