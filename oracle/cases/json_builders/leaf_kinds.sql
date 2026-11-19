select json_build_object('id', m.id, 'name', m.name, 'ratio', m.id / 4.0, 'joined', m.joined) as o from member m
